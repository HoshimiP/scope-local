use std::{
    panic,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

use ctor::ctor;
use scope_local::{Scope, scope_local};

#[ctor]
fn init() {
    percpu::init();

    unsafe { percpu::write_percpu_reg(percpu::percpu_area_base(0)) };

    let base = percpu::read_percpu_reg();
    println!("per-CPU area base = {base:#x}");
    println!("per-CPU area size = {}", percpu::percpu_area_size());
}

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

struct Counter(AtomicUsize);

impl Drop for Counter {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

scope_local! {
    static COUNTER: Counter = Counter(AtomicUsize::new(0));
}

#[test]
fn drop() {
    DROP_COUNT.store(0, Ordering::SeqCst);

    let handles: Vec<_> = (0..9)
        .map(|i| {
            thread::spawn(move || {
                let mut scope = Scope::new();
                let counter = COUNTER.scope_mut(&mut scope);
                counter.0.store(i, Ordering::SeqCst);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let panic = panic::catch_unwind(|| {
        let mut scope = Scope::new();
        let counter = COUNTER.scope_mut(&mut scope);
        counter.0.store(99, Ordering::SeqCst);
        panic!("panic");
    });
    assert!(panic.is_err());

    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 10);
}
