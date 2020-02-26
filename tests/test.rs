#![feature(test)]

use gsrs::deref_with_lifetime;
use gsrs::DerefWithLifetime;
use gsrs::SRS;

use typed_arena::Arena;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::convert::identity;
use std::hint::black_box;

struct MyBigStruct {
    f1: usize,
    _f2: Option<usize>,
}

#[derive(Default)]
struct SRSUser<'a> {
    type1: Vec<&'a MyBigStruct>,
    _type2: Vec<&'a MyBigStruct>,
}

deref_with_lifetime!(SRSUser);

fn test1(srs: SRS<Arena<MyBigStruct>, SRSUser>) -> usize {
    srs.get_ref(|user, _| user.type1[0]).f1
}

#[test]
fn test_arena() {
    let mut srs = SRS::<Arena<MyBigStruct>, SRSUser<'static>>::default();
    let a = srs.with(|data, arena| {
        let a = arena.alloc(MyBigStruct { f1: 1, _f2: None });
        data.type1.push(&*a);
        // &*a
    });

    // let b = srs.get_ref(|user,_| user.type1[0]);
    let b = test1(srs);

    // println!("{}", b);
    // drop(srs);

    assert_eq!(b, 1);
}

#[test]
fn test_create_with_and_get_ref() {
    use gsrs::*;
    struct Test {
        field: usize,
    }
    struct TestRef<'a>(&'a Test);
    deref_with_lifetime!(TestRef);

    fn test(srs: &SRS<Test, TestRef<'static>>) -> usize {
        srs.get_ref(|user, _| user.0).field
    }

    let mut srs =
        SRS::<Test, TestRef<'static>>::create_with(Test { field: 2 }, |owner| TestRef(owner));
    // let r = srs.get_ref(|user,_|user);

    let b = test(&srs);
    let mut ow = Box::new(Test { field: 0 });
    let r = srs.split(&mut ow);
    assert_eq!(2, r.0.field);
}

#[test]
fn test_new_and_get_ref() {
    use gsrs::*;
    struct Test {
        field: usize,
    }
    #[derive(Default)]
    struct TestRef<'a>(Option<&'a Test>);
    deref_with_lifetime!(TestRef);

    // fn test(srs: &SRS<Test, TestRef<'static>>) -> usize {
    //     srs.get_ref(|user,_|user).0.unwrap().field
    // }
    let mut srs = SRS::<Test, TestRef>::new(Test { field: 3 });
    srs.with(|user, owner| *user = TestRef(Some(owner)));

    // let b = test(&srs);
    let r = srs.get_ref(|user, _| user.0.unwrap());
    assert_eq!(3, r.field);
}

#[test]
fn test_vec() {
    use gsrs::*;
    struct Test {
        field: String,
    }
    struct TestRef<'a>(HashSet<&'a str>);
    deref_with_lifetime!(TestRef);
    let mut srs = SRS::<_, TestRef>::create_with(
        Test {
            field: "testtest".to_owned(),
        },
        |owner| {
            TestRef({
                let mut a = owner.field.chars();
                let mut set = HashSet::new();
                while let Some(_) = a.next() {
                    set.insert(a.as_str());
                }
                set
            })
        },
    );
    //move it whereever you like
    let mut srs = black_box(srs);
    let a = srs.with(|user, _| user.0.contains("ttest"));
    let b = srs.with(|user, _| user.0.contains("aaaaa"));
    assert!(a && !b);
}
