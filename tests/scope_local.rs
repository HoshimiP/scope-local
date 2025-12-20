use std::{panic, sync::Arc, thread};

use ctor::ctor;
use scope_local::{ActiveScope, Scope, scope_local};

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
fn global() {
    assert_eq!(*DATA, 0);
}

#[test]
fn scope() {
    let mut scope = Scope::new();
    assert_eq!(*DATA.scope(&scope), 0);

    *DATA.scope_mut(&mut scope) = 42;
    assert_eq!(*DATA.scope(&scope), 42);

    unsafe { ActiveScope::set(&scope) };
    assert_eq!(*DATA, 42);

    ActiveScope::set_global();
}

scope_local! {
    static SHARED: Arc<String> = Arc::new("qwq".to_string());
}

#[test]
fn shared() {
    assert_eq!(Arc::strong_count(&SHARED), 1);

    {
        let mut scope = Scope::new();
        *SHARED.scope_mut(&mut scope) = SHARED.clone();

        assert_eq!(Arc::strong_count(&SHARED), 2);
        assert!(Arc::ptr_eq(&SHARED, &SHARED.scope(&scope)));
    }

    assert_eq!(Arc::strong_count(&SHARED), 1);

    let panic = panic::catch_unwind(|| {
        let mut scope = Scope::new();
        *SHARED.scope_mut(&mut scope) = SHARED.clone();
        panic!("panic");
    });
    assert!(panic.is_err());

    assert_eq!(Arc::strong_count(&SHARED), 1);
}

#[test]
fn threads_shared() {
    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(move || {
                let mut scope = Scope::new();
                *SHARED.scope_mut(&mut scope) = SHARED.clone();
                assert!(Arc::strong_count(&SHARED) >= 2);
                assert_eq!(*SHARED, Arc::new("qwq".to_string()));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(Arc::strong_count(&SHARED), 1);

    {
        let mut scope = Scope::new();
        *SHARED.scope_mut(&mut scope) = SHARED.clone();
        let scope = Arc::new(scope);

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let scope = scope.clone();
                thread::spawn(move || {
                    unsafe { ActiveScope::set(&scope) };
                    assert_eq!(Arc::strong_count(&SHARED), 2);
                    assert_eq!(*SHARED, Arc::new("qwq".to_string()));
                    ActiveScope::set_global();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }

    assert_eq!(Arc::strong_count(&SHARED), 1);
}

#[test]
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
