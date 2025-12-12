use std::{
    panic,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use ctor::ctor;
use scope_local::{ActiveScope, Scope, scope_local};
use serial_test::serial;

#[ctor]
fn init() {
    percpu::init();

    unsafe { percpu::write_percpu_reg(percpu::percpu_area_base(0)) };

    let base = percpu::read_percpu_reg();
    println!("per-CPU area base = {base:#x}");
    println!("per-CPU area size = {}", percpu::percpu_area_size());
}

scope_local! {
    static DATA: usize = 0;
}

#[test]
#[serial]
fn isolation() {
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                let mut scope = Scope::new();
                *DATA.scope_mut(&mut scope) = i;

                unsafe { ActiveScope::set(&scope) };
                assert_eq!(*DATA, i);

                ActiveScope::set_global();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(*DATA, 0);
}

#[test]
#[serial]
fn nested() {
    let mut outer = Scope::new();
    unsafe { ActiveScope::set(&outer) };
    *DATA.scope_mut(&mut outer) = 1;

    let mut inner = Scope::new();
    unsafe { ActiveScope::set(&inner) };
    *DATA.scope_mut(&mut inner) = 2;
    assert_eq!(*DATA, 2);

    unsafe { ActiveScope::set(&outer) };
    assert_eq!(*DATA, 1);

    ActiveScope::set_global();
}

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[allow(dead_code)]
struct Counter;

impl Drop for Counter {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

scope_local! {
    static COUNTER: Arc<Counter> = Arc::new(Counter);
}

#[test]
#[serial]
fn drop() {
    DROP_COUNT.store(0, Ordering::SeqCst);

    let handles: Vec<_> = (0..9)
        .map(|_| {
            thread::spawn(move || {
                let _scope = Scope::new();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let panic = panic::catch_unwind(|| {
        let _scope = Scope::new();
        panic!("panic");
    });
    assert!(panic.is_err());

    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 10);
}
