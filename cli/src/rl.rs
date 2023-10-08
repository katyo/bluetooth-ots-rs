use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use futures::StreamExt;

pub trait InputFilter {
    fn handle(&self, event: &KeyEvent, line: &mut String);
}

pub struct InputReader<F> {
    events: EventStream,
    filter: F,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InputEvent {
    Ret,
    Int,
    End,
}

impl<F: InputFilter> InputReader<F> {
    pub fn new(filter: F) -> std::io::Result<Self> {
        let events = EventStream::new();

        // TODO: test terminal for interactive mode

        Ok(Self {
            events,
            filter,
            cursor: 0,
        })
    }

    pub async fn read(&mut self, line: &mut String) -> std::io::Result<InputEvent> {
        loop {
            match self.events.next().await {
                Some(Ok(Event::Key(event))) => match event {
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char(chr),
                        ..
                    } => match chr {
                        'c' => return Ok(InputEvent::Int),
                        'd' => return Ok(InputEvent::End),
                        _ => continue,
                    },
                    KeyEvent { code, .. } => match code {
                        KeyCode::Esc => return Ok(InputEvent::Int),
                        KeyCode::Enter => return Ok(InputEvent::Ret),
                        KeyCode::Backspace => {
                            line.pop();
                            continue;
                        }
                        _ => continue,
                    },
                },
                _ => (),
            }
        }
    }
}

/*
pub async fn readline_hex() -> Result<Option<Vec<u8>>> {
    let mut data = Vec::new();

    terminal::enable_raw_mode()?;
    let mut run = false;
    let mut line = String::new();
    while run {

            // mouse events etc.
            Some(Ok(_)) => continue,
            Some(Err(e)) => return (buffer, Err(e.into())),
            // TODO when is this case reached?
            None => return (buffer, Ok(())),
        }

        let (line, res) = editor.readline().await;

        if let Err(err) = res {
            terminal::disable_raw_mode()?;
            match err {
                Error::IoError(err) => Err(err)?,
                Error::Interrupted => return Ok(None),
                Error::Eof => run = false,
            }
        }

        if let Err(err) = lines.unbounded_send(format!(">> {}", line)) {
            terminal::disable_raw_mode()?;
            Err(err)?;
        }

        let mut half = None;
        for chr in line.chars() {
            if chr.is_whitespace() {
                continue;
            }
            let dig = if let Some(dig) = chr.to_digit(16) {
                dig as u8
            } else {
                terminal::disable_raw_mode()?;
                anyhow::bail!("Hexadecimal data expected");
            };
            if let Some(half) = half.take() {
                data.push(half | dig);
            } else {
                half = Some(dig << 4);
            }
        }
    }

    Ok(Some(data))
}
*/
