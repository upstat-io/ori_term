//! Benchmarks for hot-path grid operations.
//!
//! Models realistic terminal workloads: a VTE handler driving `put_char` for
//! every byte of PTY output, linefeeds triggering scroll, and bulk erases for
//! screen clears. Sizes chosen to match real usage:
//!
//! - **80x24**: Classic terminal (ssh, tmux panes).
//! - **120x50**: Modern half-screen split.
//! - **240x80**: Full-screen 4K terminal.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use oriterm_core::grid::Grid;
use oriterm_core::index::Column;
use oriterm_core::{Cell, DisplayEraseMode, LineEraseMode};

/// Terminal sizes that represent real usage.
const SIZES: [(usize, usize); 3] = [
    (80, 24),  // Classic VT100.
    (120, 50), // Modern split pane.
    (240, 80), // Full-screen 4K.
];

// ---------------------------------------------------------------------------
// Helpers: realistic content generation
// ---------------------------------------------------------------------------

/// Simulate `cat large_file.txt` — mostly ASCII with occasional wide chars.
/// This is the most common terminal workload: compiler output, logs, `ls -la`,
/// git log, etc. ~95% ASCII, ~5% CJK/emoji.
fn ascii_heavy_line(cols: usize) -> Vec<char> {
    let mut chars = Vec::with_capacity(cols);
    for i in 0..cols {
        if i % 20 == 19 {
            // Every 20th char is CJK (takes 2 columns, so line is shorter).
            chars.push('好');
        } else {
            // Printable ASCII cycling through a-z.
            chars.push((b'a' + (i % 26) as u8) as char);
        }
    }
    chars
}

/// Simulate `cat japanese_file.txt` — mostly CJK, worst case for put_char
/// because every character triggers the wide-char code path.
fn cjk_heavy_line(cols: usize) -> Vec<char> {
    // CJK unified ideographs (each width 2).
    let cjk: Vec<char> = "漢字混在表示速度測定用".chars().collect();
    let mut chars = Vec::with_capacity(cols / 2);
    for i in 0..(cols / 2) {
        chars.push(cjk[i % cjk.len()]);
    }
    chars
}

/// Pre-populate a grid with content on every line (simulates a full screen).
fn filled_grid(lines: usize, cols: usize) -> Grid {
    let mut grid = Grid::new(lines, cols);
    let line_chars = ascii_heavy_line(cols);
    for line in 0..lines {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        for &ch in &line_chars {
            grid.put_char(ch);
        }
    }
    // Reset cursor to a realistic position.
    grid.cursor_mut().set_line(lines - 1);
    grid.cursor_mut().set_col(Column(0));
    grid
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// `put_char` ASCII: the single hottest function. Called for every printable
/// byte from the PTY. This models filling a full screen of ASCII text — what
/// happens during `cat`, `gcc` output, `git log`, etc.
fn bench_put_char_ascii(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_char/ascii_line");
    for &(cols, lines) in &SIZES {
        let chars = ascii_heavy_line(cols);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines, &chars),
            |b, &(cols, lines, chars)| {
                let mut grid = Grid::new(lines, cols);
                b.iter(|| {
                    grid.cursor_mut().set_line(0);
                    grid.cursor_mut().set_col(Column(0));
                    for &ch in black_box(chars) {
                        grid.put_char(ch);
                    }
                });
            },
        );
    }
    group.finish();
}

/// `put_char` CJK: worst-case width path. Every char is width-2, triggering
/// the wide char + spacer write path. Models viewing CJK documents.
fn bench_put_char_cjk(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_char/cjk_line");
    for &(cols, lines) in &SIZES {
        let chars = cjk_heavy_line(cols);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines, &chars),
            |b, &(cols, lines, chars)| {
                let mut grid = Grid::new(lines, cols);
                b.iter(|| {
                    grid.cursor_mut().set_line(0);
                    grid.cursor_mut().set_col(Column(0));
                    for &ch in black_box(chars) {
                        grid.put_char(ch);
                    }
                });
            },
        );
    }
    group.finish();
}

/// `put_char` with wrapping: fill the full screen so every line triggers
/// an end-of-line wrap. This is what `cat large_file.txt` actually looks
/// like — continuous text flowing across lines.
fn bench_put_char_full_screen(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_char/full_screen");
    for &(cols, lines) in &SIZES {
        let chars = ascii_heavy_line(cols);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines, &chars),
            |b, &(cols, lines, chars)| {
                let mut grid = Grid::new(lines, cols);
                b.iter(|| {
                    // Fill every line, wrapping at the end of each.
                    for line in 0..lines {
                        grid.cursor_mut().set_line(line);
                        grid.cursor_mut().set_col(Column(0));
                        for &ch in black_box(chars) {
                            grid.put_char(ch);
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

/// Scroll: linefeed at the bottom line, which triggers `scroll_up`.
/// This is the second hottest path — every newline at the bottom of the
/// screen causes a scroll. Models `tail -f`, build output, `yes`.
fn bench_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/linefeed_at_bottom");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    // Cursor at bottom, linefeed triggers scroll.
                    grid.cursor_mut().set_line(lines - 1);
                    grid.linefeed();
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Scroll with BCE: same as above but cursor template has a non-default
/// background. This is the vim/tmux case where the status bar or editor
/// background means every scroll fill row needs coloring.
fn bench_scroll_bce(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/linefeed_bce");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                grid.cursor_mut().template_mut().bg = vte::ansi::Color::Indexed(4);
                b.iter(|| {
                    grid.cursor_mut().set_line(lines - 1);
                    grid.linefeed();
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Scroll down (reverse index at top): RI at top of screen inserts a blank
/// line. Less common than scroll_up but exercised by `tput ri`, some editors
/// for reverse scrolling, and cursor-up-past-top in scroll region.
fn bench_scroll_down(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/reverse_index_at_top");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    grid.cursor_mut().set_line(0);
                    grid.reverse_index();
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Scroll up within a DECSTBM sub-region: the vim/tmux hot path. Only the
/// editor area scrolls while status bars and tab lines stay fixed. Models
/// a typical split: 2 lines reserved (top bar + status), rest scrolls.
fn bench_scroll_sub_region(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/sub_region");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                // Reserve line 0 (tab bar) and last line (status bar).
                grid.set_scroll_region(2, Some(lines - 1));
                b.iter(|| {
                    grid.cursor_mut().set_line(lines - 2);
                    grid.linefeed();
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Insert lines (IL): CSI Ps L. Used by vim `O` (open line above), tmux
/// pane resize, and any editor that inserts lines mid-screen. Pushes
/// existing lines down within the scroll region.
fn bench_insert_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/insert_lines");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    grid.cursor_mut().set_line(lines / 2);
                    grid.insert_lines(black_box(1));
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Delete lines (DL): CSI Ps M. Used by vim `dd`, tmux pane close, and
/// any editor that deletes lines mid-screen. Pulls remaining lines up
/// within the scroll region.
fn bench_delete_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/delete_lines");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    grid.cursor_mut().set_line(lines / 2);
                    grid.delete_lines(black_box(1));
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Erase display (full screen clear): `clear`, `Ctrl-L`, CSI 2 J.
/// Happens frequently in interactive shells and TUI apps.
fn bench_erase_display_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("erase/display_all");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    grid.erase_display(DisplayEraseMode::All);
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Erase line below cursor: CSI 0 K. The most common line erase — used
/// by shells after every prompt to clear the rest of the line, by vim on
/// every cursor movement, by tmux to redraw status bars.
fn bench_erase_line_below(c: &mut Criterion) {
    let mut group = c.benchmark_group("erase/line_below");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                // Cursor mid-line (typical: shell prompt at col ~30).
                grid.cursor_mut().set_line(lines / 2);
                grid.cursor_mut().set_col(Column(cols / 3));
                b.iter(|| {
                    grid.erase_line(LineEraseMode::Right);
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Insert blank (ICH): CSI Ps @. Used by shells with insert mode, vim's
/// insert-before-cursor, and tmux pane redraws.
fn bench_insert_blank(c: &mut Criterion) {
    let mut group = c.benchmark_group("editing/insert_blank");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                grid.cursor_mut().set_line(lines / 2);
                grid.cursor_mut().set_col(Column(cols / 3));
                b.iter(|| {
                    // Insert 10 blanks (realistic for tab completion, etc.).
                    grid.insert_blank(black_box(10));
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Delete chars (DCH): CSI Ps P. Used by shells on backspace, vim on `x`,
/// and any editor that deletes in the middle of a line.
fn bench_delete_chars(c: &mut Criterion) {
    let mut group = c.benchmark_group("editing/delete_chars");
    for &(cols, lines) in &SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines),
            |b, &(cols, lines)| {
                let mut grid = filled_grid(lines, cols);
                grid.cursor_mut().set_line(lines / 2);
                grid.cursor_mut().set_col(Column(cols / 3));
                b.iter(|| {
                    grid.delete_chars(black_box(10));
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Row reset: the primitive inside scroll fill and erase_display. Benchmarks
/// the occupancy-aware fast path vs the full-row BCE path.
fn bench_row_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("row/reset");
    for &(cols, _) in &SIZES {
        // Reset a dirty row (occ > 0) with default template.
        group.bench_with_input(
            BenchmarkId::new("dirty_default", cols),
            &cols,
            |b, &cols| {
                let tmpl = Cell::default();
                let chars = ascii_heavy_line(cols);
                let mut row = oriterm_core::grid::Row::new(cols);
                // Dirty the row.
                for (i, &ch) in chars.iter().enumerate().take(cols) {
                    row[Column(i)].ch = ch;
                }
                b.iter(|| {
                    row.reset(black_box(cols), black_box(&tmpl));
                });
            },
        );
        // Reset a dirty row with BCE template (forces full repaint).
        group.bench_with_input(BenchmarkId::new("dirty_bce", cols), &cols, |b, &cols| {
            let tmpl = Cell::from(vte::ansi::Color::Indexed(4));
            let chars = ascii_heavy_line(cols);
            let mut row = oriterm_core::grid::Row::new(cols);
            for (i, &ch) in chars.iter().enumerate().take(cols) {
                row[Column(i)].ch = ch;
            }
            b.iter(|| {
                row.reset(black_box(cols), black_box(&tmpl));
            });
        });
        // Reset a clean row (occ = 0) — the fast path.
        group.bench_with_input(
            BenchmarkId::new("clean_default", cols),
            &cols,
            |b, &cols| {
                let tmpl = Cell::default();
                let mut row = oriterm_core::grid::Row::new(cols);
                b.iter(|| {
                    row.reset(black_box(cols), black_box(&tmpl));
                });
            },
        );
    }
    group.finish();
}

/// Realistic terminal session: simulates what happens when a compiler
/// spews output — mostly ASCII text, every line ends with a linefeed that
/// may trigger scroll, occasional clear-to-end-of-line.
fn bench_realistic_output_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic/output_burst");
    for &(cols, lines) in &SIZES {
        let text_line = ascii_heavy_line(cols);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines, &text_line),
            |b, &(cols, lines, chars)| {
                let mut grid = Grid::new(lines, cols);
                b.iter(|| {
                    // Write 100 lines of output (typical compiler burst).
                    for _ in 0..100 {
                        grid.cursor_mut().set_col(Column(0));
                        for &ch in black_box(chars) {
                            grid.put_char(ch);
                        }
                        grid.carriage_return();
                        grid.linefeed();
                    }
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

/// Realistic TUI redraw: simulates what vim/tmux does on each keypress.
/// Cursor moves, partial line erases, write new content. This is the
/// interactive latency-sensitive path.
fn bench_realistic_tui_redraw(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic/tui_redraw");
    for &(cols, lines) in &SIZES {
        let text_line = ascii_heavy_line(cols);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{cols}x{lines}")),
            &(cols, lines, &text_line),
            |b, &(cols, lines, chars)| {
                let mut grid = filled_grid(lines, cols);
                b.iter(|| {
                    // Redraw 10 lines (typical partial TUI update).
                    for i in 0..10 {
                        let line = i % lines;
                        grid.cursor_mut().set_line(line);
                        grid.cursor_mut().set_col(Column(0));
                        // Erase to end of line, then rewrite.
                        grid.erase_line(LineEraseMode::Right);
                        for &ch in black_box(chars) {
                            grid.put_char(ch);
                        }
                    }
                    black_box(&grid);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_put_char_ascii,
    bench_put_char_cjk,
    bench_put_char_full_screen,
    bench_scroll,
    bench_scroll_bce,
    bench_scroll_down,
    bench_scroll_sub_region,
    bench_insert_lines,
    bench_delete_lines,
    bench_erase_display_all,
    bench_erase_line_below,
    bench_insert_blank,
    bench_delete_chars,
    bench_row_reset,
    bench_realistic_output_burst,
    bench_realistic_tui_redraw,
);
criterion_main!(benches);
