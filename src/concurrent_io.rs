use std::{collections::HashMap, sync::Arc};

enum Output {
    Stdout(String),
    Stderr(String)
}

pub struct ConcurrentIO {
    buffers: HashMap<Arc<str>, Vec<Output>>,
    finished_keys: Vec<Arc<str>>,
    active_key: Option<Arc<str>>
}

impl ConcurrentIO {
    pub fn new() -> ConcurrentIO {
        ConcurrentIO {
            buffers: HashMap::new(),
            finished_keys: Vec::new(),
            active_key: None
        }
    }

    pub fn print_stdout(&mut self, key: &Arc<str>, text: String) {
        if self.finished_keys.contains(key) {
            return;
        }

        let is_active = match &self.active_key {
            Some(active_key) => active_key == key,
            None => {
                self.active_key = Some(key.clone());
                true
            }
        };
        
        if is_active {
            print!("{}", text);
        }else {
            let buffer = self.buffers.entry(key.clone()).or_default();
            buffer.push(Output::Stdout(text));
        }
    }

    pub fn print_stderr(&mut self, key: &Arc<str>, text: String) {
        if self.finished_keys.contains(key) {
            return;
        }

        let is_active = match &self.active_key {
            Some(active_key) => active_key == key,
            None => {
                self.active_key = Some(key.clone());
                true
            }
        };
        
        if is_active {
            eprint!("{}", text);
        }else {
            let buffer = self.buffers.entry(key.clone()).or_default();
            buffer.push(Output::Stderr(text));
        }
    }

    pub fn finish_key(&mut self, key: &Arc<str>) {
        let is_active = match &self.active_key {
            Some(active_key) => active_key == key,
            None => false
        };
        
        if is_active {
            self.flush_buffer(key);
            self.active_key = None;
        } else {
            self.finished_keys.push(key.clone());
        }
        
        for buf_key in self.get_flushable_buffers() {
            self.flush_buffer(&buf_key);
        }
    }

    pub fn get_keys(&self) -> Vec<&Arc<str>> {
        self.buffers.keys().collect()
    }

    fn get_flushable_buffers(&self) -> Vec<Arc<str>> {
        match self.active_key {
            None => self.finished_keys.clone(),
            Some(_) => Vec::new()
        }
    }

    fn flush_buffer(&mut self, key: &Arc<str>) {
        if self.active_key.is_some() {
            return;
        }

        if let Some(buffer) = self.buffers.get_mut(key) {
            for output in buffer.drain(..) {
                match output {
                    Output::Stdout(s) => { print!("{}", s); }
                    Output::Stderr(s) => { eprint!("{}", s); }
                }
            }
        }
    }
}

impl Drop for ConcurrentIO {
    fn drop(&mut self) {
        if let Some(active_key) = self.active_key.clone() {
            self.finish_key(&active_key);
        } else {
            for buf_key in self.get_flushable_buffers() {
                self.flush_buffer(&buf_key);
            }
        }
    }
}

