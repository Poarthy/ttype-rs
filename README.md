# ttype (Rust port)

`ttype` is a terminal typing trainer with prose and code modes, replay, live
WPM charts, consistency scores, and character-error heatmaps. This project is
the safe-Rust port of the Go reference in `/root/ttype`.

The final application uses a single-threaded `ratatui` and `crossterm` event
loop, stores user configuration as TOML, and embeds shipped practice lists so
normal runs work offline. Network access is limited to the optional update
check.
