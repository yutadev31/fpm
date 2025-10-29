macro_rules! info {
    ($($arg:tt)*) => {{
        use crossterm::style::{Color, SetForegroundColor};

        println!(
            "{}:: {}{}",
            SetForegroundColor(Color::Green),
            format_args!($($arg)*),
            SetForegroundColor(Color::Reset)
        );
    }};
}

macro_rules! warning {
    ($($arg:tt)*) => {{
        use crossterm::style::{Color, SetForegroundColor};

        println!(
            "{}:: {}{}",
            SetForegroundColor(Color::Yellow),
            format_args!($($arg)*),
            SetForegroundColor(Color::Reset)
        );
    }};
}

macro_rules! error {
    ($($arg:tt)*) => {{
        use crossterm::style::{Color, SetForegroundColor};

        println!(
            "{}Error: {}{}",
            SetForegroundColor(Color::Red),
            format_args!($($arg)*),
            SetForegroundColor(Color::Reset)
        );
    }};
}

pub(crate) use error;
pub(crate) use info;
pub(crate) use warning;
