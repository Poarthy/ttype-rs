use ttype_core::storage::{PersonalBests, StatsSummary};
use ttype_core::tui::render_stats_text;

#[test]
fn stats_screen_matches_go_empty_and_filtered_messages() {
    assert_eq!(
        render_stats_text(&StatsSummary::default(), &[], 72),
        "No test history yet.\n"
    );
    let filtered = StatsSummary {
        filter_mode: "sql".to_owned(),
        ..StatsSummary::default()
    };
    assert_eq!(
        render_stats_text(&filtered, &[], 72),
        "No tests recorded for mode \"sql\".\n"
    );
}

#[test]
fn stats_screen_renders_summary_personal_best_and_trend() {
    let summary = StatsSummary {
        total_tests: 3,
        average_wpm: 61.5,
        average_accuracy: 94.25,
        best_wpm: 70.0,
        recent_average_wpm: 62.0,
        personal_best: PersonalBests {
            best_wpm: 70.0,
            best_accuracy: 99.0,
            updated_at: String::new(),
        },
        ..StatsSummary::default()
    };
    let output = render_stats_text(&summary, &[40.0, 55.0, 70.0], 72);
    for expected in ["All tests", "61.50", "94.25%", "70.00", "all-time", "trend"] {
        assert!(output.contains(expected), "missing {expected:?}: {output}");
    }
}

#[test]
fn filtered_stats_hide_global_personal_bests() {
    let summary = StatsSummary {
        total_tests: 2,
        filter_mode: "sql".to_owned(),
        best_wpm: 40.0,
        personal_best: PersonalBests {
            best_wpm: 120.0,
            ..PersonalBests::default()
        },
        ..StatsSummary::default()
    };
    assert!(!render_stats_text(&summary, &[], 72).contains("all-time"));
}
