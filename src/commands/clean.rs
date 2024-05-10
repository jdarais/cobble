use std::{path::Path, process::ExitCode};



pub struct CleanCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>
}

pub fn clean_command<'a>(input: CleanCommandInput<'a>) -> ExitCode {

    return ExitCode::from(0);
}