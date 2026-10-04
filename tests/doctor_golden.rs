use ttype_core::doctor::{Check, color_with, locale_with, render};

#[test]
fn locale_and_color_follow_go_environment_rules() {
    assert!(locale_with(["".to_owned(), "".to_owned(), "en_US.UTF-8".to_owned()]).ok);
    assert!(!locale_with(["".to_owned(), "".to_owned(), "en_US.ISO-8859-1".to_owned()]).ok);

    assert!(!color_with("dumb".to_owned(), false).ok);
    assert!(color_with("xterm-256color".to_owned(), false).ok);
}

#[test]
fn rendered_checks_count_the_failures() {
    let (output, failed) = render(&[
        Check {
            name: "one",
            ok: true,
            note: "fine".to_owned(),
            hint: String::new(),
        },
        Check {
            name: "two",
            ok: false,
            note: "broken".to_owned(),
            hint: "try this".to_owned(),
        },
    ]);
    assert_eq!(failed, 1);
    assert!(output.contains("[ok  ] one"));
    assert!(output.contains("[fail] two"));
    assert!(output.contains("try this"));
}
