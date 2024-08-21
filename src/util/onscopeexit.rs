// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

pub struct OnScopeExit {
    on_scope_exit: Option<Box<dyn FnOnce() -> ()>>
}

impl OnScopeExit {
    pub fn new(on_scope_exit: Box<dyn FnOnce() -> ()>) -> OnScopeExit {
        OnScopeExit { on_scope_exit: Some(on_scope_exit) }
    }
}

impl Drop for OnScopeExit {
    fn drop(&mut self) {
        let func = self.on_scope_exit.take().unwrap();
        func();
    }
}


pub struct OnScopeExitMut<'a, T> {
    inner: &'a mut T,
    on_scope_exit: Option<Box<dyn FnOnce(&mut T) -> ()>>,
}

impl<'a, T> OnScopeExitMut<'a, T> {
    pub fn new(
        val: &'a mut T,
        on_scope_exit: Box<dyn FnOnce(&mut T) -> ()>,
    ) -> OnScopeExitMut<'a, T> {
        OnScopeExitMut {
            inner: val,
            on_scope_exit: Some(on_scope_exit),
        }
    }
}

impl<'a, T> AsMut<T> for OnScopeExitMut<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<'a, T> Drop for OnScopeExitMut<'a, T> {
    fn drop(&mut self) {
        let func = self.on_scope_exit.take().unwrap();
        func(self.inner);
    }
}
