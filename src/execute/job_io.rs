use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
};

use crate::{config::TaskOutputCondition, execute::execute::TaskResult};

enum Output {
    Status(String),
    Stdout(String),
    Stderr(String),
}

#[derive(Copy, Clone)]
enum TrackedJobState {
    InProgress,
    Complete,
    Closed,
}

struct TrackedJob {
    job_state: TrackedJobState,
    buffer: Vec<Output>,
    on_fail_buffer: Vec<Output>,
    stdin_ready: Arc<(Mutex<bool>, Condvar)>,
    show_stdout: TaskOutputCondition,
    show_stderr: TaskOutputCondition,
    failed: bool
}

pub struct ConcurrentIO {
    jobs: HashMap<Arc<str>, TrackedJob>,
    active_job: Option<Arc<str>>,
}

impl ConcurrentIO {
    pub fn new() -> ConcurrentIO {
        ConcurrentIO {
            jobs: HashMap::new(),
            active_job: None,
        }
    }

    pub fn job_started(
        &mut self,
        job_id: &Arc<str>,
        stdin_ready: Arc<(Mutex<bool>, Condvar)>,
        show_stdout: TaskOutputCondition,
        show_stderr: TaskOutputCondition,
    ) {
        self.jobs.insert(
            job_id.clone(),
            TrackedJob {
                job_state: TrackedJobState::InProgress,
                buffer: Vec::new(),
                on_fail_buffer: Vec::new(),
                stdin_ready,
                show_stdout,
                show_stderr,
                failed: false
            },
        );
        self.print_status(&job_id, format!("-----> {}\n", job_id));
        self.update_active_job();
    }

    pub fn print_status(&mut self, job_id: &Arc<str>, text: String) {
        let is_active = match &self.active_job {
            Some(active_job_id) => active_job_id == job_id,
            None => false,
        };

        let job_opt = self.jobs.get_mut(job_id);
        if let Some(job) = job_opt {
            if is_active {
                print!("{}", text);
            } else {
                if let TrackedJobState::Complete = job.job_state {
                    return;
                }

                job.buffer.push(Output::Status(text));
            }
        }
    }

    pub fn print_stdout(&mut self, job_id: &Arc<str>, text: String) {
        let is_active = match &self.active_job {
            Some(active_job_id) => active_job_id == job_id,
            None => false,
        };

        let job_opt = self.jobs.get_mut(job_id);
        if let Some(job) = job_opt {
            if is_active {
                match job.show_stdout {
                    TaskOutputCondition::Always => { print!("{}", text); }
                    TaskOutputCondition::OnFail => { job.on_fail_buffer.push(Output::Stdout(text)); }
                    TaskOutputCondition::Never => { /* Ignore */ }
                }
            } else {
                if let TrackedJobState::Complete = job.job_state {
                    return;
                }

                job.buffer.push(Output::Stdout(text));
            }
        }
    }

    pub fn print_stderr(&mut self, job_id: &Arc<str>, text: String) {
        let is_active = match &self.active_job {
            Some(active_job_id) => active_job_id == job_id,
            None => false,
        };

        let job_opt = self.jobs.get_mut(job_id);
        if let Some(job) = job_opt {
            if is_active {
                match job.show_stdout {
                    TaskOutputCondition::Always => { eprint!("{}", text); }
                    TaskOutputCondition::OnFail => { job.on_fail_buffer.push(Output::Stderr(text)); }
                    TaskOutputCondition::Never => { /* Ignore */}
                } 
            } else {
                if let TrackedJobState::Complete = job.job_state {
                    return;
                }

                job.buffer.push(Output::Stderr(text));
            }
        }
    }

    pub fn job_completed(&mut self, job_id: &Arc<str>, task_result: &TaskResult) {
        match task_result {
            TaskResult::UpToDate => {
                self.print_status(&job_id, format!("[UPTD] {}\n", job_id));
            }
            TaskResult::Success => {
                self.print_status(&job_id, format!("[ OK ] {}\n", job_id));
            }
            TaskResult::Error(e) => {
                self.print_status(&job_id, format!("[FAIL] {}: {}\n", job_id, e));
            }
        }

        let job_state_opt = self.jobs.get(job_id).map(|j| j.job_state);

        if let Some(job_state) = job_state_opt {
            if let TrackedJobState::Closed = job_state {
                return;
            }
            let job = self.jobs.get_mut(job_id).unwrap();
            job.job_state = TrackedJobState::Complete;
            job.failed = match task_result { TaskResult::Error(_) => true, _ => false };
            self.update_active_job();
        }
    }

    fn update_active_job(&mut self) {
        match self.active_job.clone() {
            Some(active_job) => {
                match self.jobs.get(&active_job).map(|j| j.job_state) {
                    Some(job_state) => {
                        match job_state {
                            TrackedJobState::Complete | TrackedJobState::Closed => {
                                self.flush_buffer(&active_job);
                                self.jobs.get_mut(&active_job).unwrap().job_state =
                                    TrackedJobState::Closed;
                                self.active_job = None;
                                self.update_active_job();
                            }
                            TrackedJobState::InProgress => { /* Nothing to do */ }
                        }
                    }
                    None => {
                        panic!("JobIO active_id {} is not in the list of jobs", active_job);
                    }
                }
            }
            None => {
                // Find a new active job ID
                let mut new_active_job_id: Option<Arc<str>> = None;
                for (job_id, job) in &self.jobs {
                    match job.job_state {
                        TrackedJobState::InProgress | TrackedJobState::Complete => {
                            new_active_job_id = Some(job_id.clone());
                            break;
                        }
                        TrackedJobState::Closed => { /* Ignore */ }
                    };
                }
                if let Some(id) = new_active_job_id {
                    self.flush_buffer(&id);

                    let (ready_lock, ready_condvar) = &*self.jobs.get(&id).unwrap().stdin_ready;
                    *ready_lock.lock().unwrap() = true;
                    ready_condvar.notify_all();

                    self.active_job = Some(id);
                    self.update_active_job();
                }
            }
        }
    }

    fn flush_buffer(&mut self, key: &Arc<str>) {
        if let Some(job) = self.jobs.get_mut(key) {
            for output in job.buffer.drain(..) {
                match output {
                    Output::Stdout(s) => match job.show_stdout {
                        TaskOutputCondition::Always => { print!("{}", s); }
                        TaskOutputCondition::OnFail => { job.on_fail_buffer.push(Output::Stdout(s)); }
                        TaskOutputCondition::Never => { /* Ignore */}
                    }
                    Output::Stderr(s) => match job.show_stderr {
                        TaskOutputCondition::Always => { eprint!("{}", s); }
                        TaskOutputCondition::OnFail => { job.on_fail_buffer.push(Output::Stderr(s)); }
                        TaskOutputCondition::Never => { /* Ignore */ }
                    }
                    Output::Status(s) => {
                        print!("{}", s);
                    }
                }
            }
            if job.failed {
                for output in job.on_fail_buffer.drain(..) {
                    match output {
                        Output::Stdout(s) => { print!("{}", s); }
                        Output::Stderr(s) => { eprint!("{}", s); }
                        Output::Status(s) => { print!("{}", s); }
                    }
                }
            }
        }
    }
}

impl Drop for ConcurrentIO {
    fn drop(&mut self) {
        for (_job_id, job) in self.jobs.iter_mut() {
            match job.job_state {
                TrackedJobState::Closed => { /* Do nothing */ }
                _ => {
                    job.job_state = TrackedJobState::Complete;
                }
            };
        }
        self.update_active_job();
    }
}
