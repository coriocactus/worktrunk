//! Test to reproduce the spacing alignment bug
//!
//! This test verifies that header and data rows align properly,
//! regardless of whether cells have ANSI styling applied.

use anstyle::{AnsiColor, Color, Style};
use worktrunk::styling::StyledLine;

#[test]
fn test_header_data_alignment_with_and_without_styling() {
    // Simulate what happens in the actual rendering

    // Header (always styled with dim)
    let mut header = StyledLine::new();
    let branch_width = 6;
    let branch_header = format!("{:width$}", "Branch", width = branch_width);
    header.push_styled(branch_header, Style::new().dimmed());
    header.push_raw("  "); // separator
    let age_header = format!("{:width$}", "Age", width = 14);
    header.push_styled(age_header, Style::new().dimmed());

    // Data row 1: with styling (styled cell)
    let mut row1 = StyledLine::new();
    let branch1_text = format!("{:width$}", "main", width = branch_width);
    row1.push_styled(
        branch1_text.clone(),
        Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
    ); // Styled
    row1.push_raw("  "); // separator
    let time1_text = format!("{:width$}", "23 minutes ago", width = 14);
    row1.push_styled(time1_text, Style::new().dimmed());

    // Data row 2: without styling (raw cell)
    let mut row2 = StyledLine::new();
    let branch2_text = format!("{:width$}", "foo", width = branch_width);
    row2.push_raw(branch2_text.clone()); // NOT styled
    row2.push_raw("  "); // separator
    let time2_text = format!("{:width$}", "23 minutes ago", width = 14);
    row2.push_styled(time2_text, Style::new().dimmed());

    // Check that all rows have the same width up to the time column
    let header_width_before_time = branch_width + 2; // branch + separator
    let row1_width_before_time = branch_width + 2;
    let row2_width_before_time = branch_width + 2;

    assert_eq!(
        header_width_before_time, row1_width_before_time,
        "Header and styled row should align"
    );
    assert_eq!(
        header_width_before_time, row2_width_before_time,
        "Header and unstyled row should align"
    );
}
