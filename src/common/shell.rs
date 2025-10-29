use std::{
    io::{Stdout, stdout},
    process::{Stdio, exit},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

pub async fn run_shell(command: &str, fakeroot: bool) -> anyhow::Result<()> {
    let mut child = if fakeroot {
        Command::new("fakeroot")
            .arg("bash")
            .arg("-eux")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
    } else {
        Command::new("bash")
            .arg("-eux")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
    };

    let stdout_reader = BufReader::new(child.stdout.take().unwrap());
    let stderr_reader = BufReader::new(child.stderr.take().unwrap());

    let mut stdout_lines = stdout_reader.lines();
    let mut stderr_lines = stderr_reader.lines();

    let mut stdout = stdout();

    fn draw(stdout: &mut Stdout, line: &str, color: Color) -> anyhow::Result<()> {
        execute!(
            stdout,
            Clear(ClearType::CurrentLine),
            SetForegroundColor(color),
            Print(format!("{}\n", line)),
            ResetColor
        )?;

        Ok(())
    }

    loop {
        tokio::select! {
            line = stdout_lines.next_line() => {
                if let Ok(Some(line)) = line {
                    draw(&mut stdout, &line, Color::Reset)?;
                } else {
                    break;
                }
            }
            line = stderr_lines.next_line() => {
                if let Ok(Some(line)) = line {
                    if line.starts_with("+ ") {
                        let current_command = line.trim_start_matches("+ ").trim().to_string();
                        let line = format!("$ {}", current_command);
                        draw(&mut stdout, &line, Color::Blue)?;
                    } else if !line.starts_with('+') {
                        draw(&mut stdout, &line, Color::Yellow)?;
                    }

                } else {
                    break;
                }
            }
        }
    }

    tokio::spawn(async {
        loop {
            let evt = event::read()?;

            if let Event::Key(evt) = evt
                && let KeyCode::Char('c') = evt.code
                && evt.modifiers.contains(KeyModifiers::CONTROL)
            {
                break;
            }
        }

        terminal::disable_raw_mode()?;
        exit(1);

        #[allow(unreachable_code)]
        anyhow::Ok(())
    });

    let status = child.wait().await?;

    execute!(stdout, Clear(ClearType::CurrentLine))?;
    terminal::disable_raw_mode()?;

    if !status.success() {
        anyhow::bail!("Script failed: {}", command);
    }

    Ok(())
}
