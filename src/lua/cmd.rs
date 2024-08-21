// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;

use mlua::{Error, Function, Lua, Table, UserData, Value};

use crate::lua::lua_env::COBBLE_JOB_INTERACTIVE_ENABLED;

pub struct CmdLib;

impl UserData for CmdLib {
    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("cmd", exec_shell_command);
    }
}

enum ChildMessage {
    Stdout(String),
    StdoutDone,
    Stderr(String),
    StderrDone,
}

fn exec_shell_command<'lua>(lua: &'lua Lua, args: Table<'lua>) -> mlua::Result<Table<'lua>> {
    let args_len_int = args.len()?;
    let args_len: usize = args_len_int
        .try_into()
        .map_err(|e| Error::runtime(format!("Invalid indices used for command args: {}", e)))?;

    let mut cmd_with_args: Vec<String> = Vec::with_capacity(args_len);
    cmd_with_args.resize(args_len, String::new());

    let mut cwd: Option<PathBuf> = None;
    let mut out_func: Option<Function> = None;
    let mut err_func: Option<Function> = None;

    for pair in args.pairs() {
        let (k, v): (Value, Value) = pair?;
        match k {
            Value::Integer(i) => {
                let idx: usize = i.try_into().map_err(|e| {
                    Error::runtime(format!("Invalid index used for command args: {}", e))
                })?;
                if idx > cmd_with_args.len() {
                    return Err(Error::runtime(format!(
                        "Invalid index used for command args: {}",
                        idx
                    )));
                }
                cmd_with_args[idx - 1] = lua.unpack(v)?;
            }
            Value::String(s) => {
                let s_str = s.to_str().map_err(|e| {
                    Error::runtime(format!("Error reading lua string value: {}", e))
                })?;
                match s_str {
                    "cwd" => {
                        cwd = Some(PathBuf::from(lua.unpack::<String>(v)?));
                    }
                    "out" => {
                        out_func = Some(lua.unpack(v)?);
                    }
                    "err" => {
                        err_func = Some(lua.unpack(v)?);
                    }
                    _ => {
                        return Err(Error::runtime(format!(
                            "Unknown key in cmd input: {}",
                            s.to_str().unwrap_or("<error reading value>")
                        )));
                    }
                };
            }
            _ => {
                return Err(Error::runtime(format!(
                    "Key type not allowed in cmd input: {}",
                    k.type_name()
                )));
            }
        };
    }

    if cmd_with_args.len() < 1 {
        return Err(Error::runtime("No command given"));
    }

    let cmd_cmd = &cmd_with_args[0];
    let cmd_args = &cmd_with_args[1..];

    let mut cmd = Command::new(cmd_cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.args(cmd_args);

    if let Some(d) = cwd {
        cmd.current_dir(d);
    }

    let interactive_enabled: bool = lua.named_registry_value(COBBLE_JOB_INTERACTIVE_ENABLED)?;
    if interactive_enabled {
        cmd.stdin(Stdio::inherit());
    } else {
        cmd.stdin(Stdio::null());
    }

    let child_res = cmd.spawn();

    match child_res {
        Err(e) => Err(Error::runtime(format!("Error executing command '{} {}': {}", cmd_cmd, cmd_args.join(" "), e))),
        Ok(mut child) => {
            let (tx, rx) = channel();

            let stdout_tx = tx.clone();
            let mut stdout = child.stdout.take().unwrap();
            let stdout_thread = thread::spawn(move || {
                let mut buf: Vec<u8> = Vec::with_capacity(256);
                buf.resize(256, 0);
                let mut start_from = 0usize;
                loop {
                    let res = stdout.read(&mut buf[start_from..]);
                    match res {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                break;
                            }
                            match std::str::from_utf8(&buf[start_from..(start_from+bytes_read)]) {
                                Ok(out) => {
                                    stdout_tx.send(ChildMessage::Stdout(String::from(out))).unwrap();
                                    start_from = 0;
                                },
                                Err(_) => {
                                    start_from = start_from + bytes_read;
                                    if start_from >= buf.len() {
                                        buf.resize(buf.len()*2, 0);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
                stdout_tx.send(ChildMessage::StdoutDone).unwrap();
            });

            let stderr_tx = tx.clone();
            let mut stderr = child.stderr.take().unwrap();
            let stderr_thread = thread::spawn(move || {
                let mut buf: Vec<u8> = Vec::with_capacity(256);
                buf.resize(256, 0);
                let mut start_from = 0usize;
                loop {
                    let res = stderr.read(&mut buf);
                    match res {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                break;
                            }
                            match std::str::from_utf8(&buf[start_from..(start_from+bytes_read)]) {
                                Ok(err) => {
                                    stderr_tx.send(ChildMessage::Stderr(String::from(err))).unwrap();
                                    start_from = 0;
                                },
                                Err(_) => {
                                    start_from = start_from + bytes_read;
                                    if start_from >= buf.len() {
                                        buf.resize(buf.len()*2, 0);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
                stderr_tx.send(ChildMessage::StderrDone).unwrap();
            });

            let mut stdout_buf = String::new();
            let mut stdout_done = false;

            let mut stderr_buf = String::new();
            let mut stderr_done = false;

            while !stdout_done || !stderr_done {
                let msg = rx.recv().unwrap();

                match msg {
                    ChildMessage::Stdout(out) => {
                        stdout_buf.push_str(out.as_str());
                        if let Some(out_fn) = &out_func {
                            out_fn.call(out)?;
                        }
                    }
                    ChildMessage::StdoutDone => {
                        stdout_done = true;
                    }
                    ChildMessage::Stderr(err) => {
                        stderr_buf.push_str(err.as_str());
                        if let Some(err_fn) = &err_func {
                            err_fn.call(err)?;
                        }
                    }
                    ChildMessage::StderrDone => {
                        stderr_done = true;
                    }
                }
            }

            stdout_thread.join().unwrap();
            stderr_thread.join().unwrap();

            let status_res = child.wait();
            let status = match status_res {
                Ok(status) => lua.pack(status.code())?,
                Err(e) => {
                    return Err(Error::runtime(format!("{}", e)));
                }
            };

            let stdout = lua.create_string(stdout_buf)?;
            let stderr = lua.create_string(stderr_buf)?;

            let result = lua.create_table()?;
            result.set("stdout", stdout)?;
            result.set("stderr", stderr)?;
            result.set("status", status)?;
            Ok(result)
        }
    }
}
