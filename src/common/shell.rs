use std::{
    io::{Stdout, stdout},
    process::{Stdio, exit},
    sync::Arc,
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::Mutex,
};

pub async fn run_shell(description: &str, command: &str, fakeroot: bool) -> anyhow::Result<()> {
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

    let current_command = Arc::new(Mutex::new(String::new()));

    terminal::enable_raw_mode()?;

    fn draw(
        stdout: &mut Stdout,
        description: &str,
        line: &str,
        current_command: &str,
        color: Color,
    ) -> anyhow::Result<()> {
        execute!(
            stdout,
            Clear(ClearType::CurrentLine),
            SetForegroundColor(color),
            Print(format!("{}\n", line)),
            cursor::MoveDown(1),
            cursor::MoveToColumn(0),
            SetForegroundColor(Color::Blue),
            Print(format!("{}: ", description)),
            Print(current_command),
            SetForegroundColor(Color::Reset),
            cursor::MoveToColumn(0),
            ResetColor
        )?;

        Ok(())
    }

    loop {
        tokio::select! {
            line = stdout_lines.next_line() => {
                if let Ok(Some(line)) = line {
                    let  current_command = current_command.lock().await;
                    draw(&mut stdout, description, &line, &current_command, Color::Reset)?;
                } else {
                    break;
                }
            }
            line = stderr_lines.next_line() => {
                if let Ok(Some(line)) = line {
                    let mut current_command = current_command.lock().await;
                    if line.starts_with("+ ") {
                        *current_command = line.trim_start_matches("+ ").trim().to_string();
                        let line = format!("$ {}", current_command);
                        draw(&mut stdout, description, &line, &current_command, Color::Blue)?;
                    } else if !line.starts_with('+') {
                        draw(&mut stdout, description, &line, &current_command, Color::Yellow)?;
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
