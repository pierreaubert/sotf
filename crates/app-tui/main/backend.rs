use ratatui::backend::{Backend, ClearType, CrosstermBackend, TestBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use std::io::IsTerminal;
use std::io::{self, Stdout};

fn into_io_result<T>(result: Result<T, std::convert::Infallible>) -> io::Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(never) => match never {},
    }
}

pub(super) enum RuntimeBackend {
    Terminal(CrosstermBackend<Stdout>),
    Headless(TestBackend),
}

impl RuntimeBackend {
    pub(super) fn new(headless: bool) -> Self {
        if headless {
            Self::Headless(TestBackend::new(144, 50))
        } else {
            Self::Terminal(CrosstermBackend::new(io::stdout()))
        }
    }
}

#[cfg(any(feature = "dev-api", test))]
fn parse_dev_api_port(value: &str) -> io::Result<u16> {
    let port = value.parse::<u16>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("SOTF_DEV_API_PORT must be a valid TCP port, got `{value}`"),
        )
    })?;
    if port == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOTF_DEV_API_PORT must be greater than zero",
        ));
    }
    Ok(port)
}

pub(super) fn requested_dev_api_port() -> io::Result<Option<u16>> {
    #[cfg(feature = "dev-api")]
    {
        match std::env::var("SOTF_DEV_API_PORT") {
            Ok(value) => parse_dev_api_port(&value).map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SOTF_DEV_API_PORT must contain valid Unicode",
            )),
        }
    }

    #[cfg(not(feature = "dev-api"))]
    {
        Ok(None)
    }
}

fn should_run_headless_dev_api_for(dev_api_port: Option<u16>, stdout_is_terminal: bool) -> bool {
    dev_api_port.is_some() && !stdout_is_terminal
}

pub(super) fn should_run_headless_dev_api(dev_api_port: Option<u16>) -> bool {
    should_run_headless_dev_api_for(dev_api_port, io::stdout().is_terminal())
}

impl Backend for RuntimeBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        match self {
            Self::Terminal(backend) => backend.draw(content),
            Self::Headless(backend) => into_io_result(backend.draw(content)),
        }
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.append_lines(n),
            Self::Headless(backend) => into_io_result(backend.append_lines(n)),
        }
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.hide_cursor(),
            Self::Headless(backend) => into_io_result(backend.hide_cursor()),
        }
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.show_cursor(),
            Self::Headless(backend) => into_io_result(backend.show_cursor()),
        }
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        match self {
            Self::Terminal(backend) => backend.get_cursor_position(),
            Self::Headless(backend) => into_io_result(backend.get_cursor_position()),
        }
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let position = position.into();
        match self {
            Self::Terminal(backend) => backend.set_cursor_position(position),
            Self::Headless(backend) => into_io_result(backend.set_cursor_position(position)),
        }
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.clear(),
            Self::Headless(backend) => into_io_result(backend.clear()),
        }
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.clear_region(clear_type),
            Self::Headless(backend) => into_io_result(backend.clear_region(clear_type)),
        }
    }

    fn size(&self) -> Result<Size, Self::Error> {
        match self {
            Self::Terminal(backend) => backend.size(),
            Self::Headless(backend) => into_io_result(backend.size()),
        }
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        match self {
            Self::Terminal(backend) => backend.window_size(),
            Self::Headless(backend) => into_io_result(backend.window_size()),
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Terminal(backend) => backend.flush(),
            Self::Headless(backend) => into_io_result(backend.flush()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_api_port_validation_rejects_invalid_values() {
        assert_eq!(parse_dev_api_port("12345").unwrap(), 12345);
        assert!(parse_dev_api_port("0").is_err());
        assert!(parse_dev_api_port("invalid").is_err());
        assert!(parse_dev_api_port("70000").is_err());
    }

    #[test]
    fn headless_mode_requires_both_a_port_and_redirected_stdout() {
        assert!(should_run_headless_dev_api_for(Some(12345), false));
        assert!(!should_run_headless_dev_api_for(None, false));
        assert!(!should_run_headless_dev_api_for(Some(12345), true));
    }
}
