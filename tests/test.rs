// #![feature(test)]

use gsrs::deref_with_lifetime;
use gsrs::DerefWithLifetime;
use gsrs::SRS;

use std::cmp::Ordering;
use std::cell::Cell;
use std::ops::Deref;

#[test]
fn test_create_with_and_get_ref() {
    use gsrs::*;
    struct Test {
        field: usize,
        field2: usize,
    }
    struct TestRef<'a>(&'a Test);
    deref_with_lifetime!(TestRef);

    fn test(srs: &SRS<Test, TestRef>) -> usize {
        srs.get_ref(|user, _| user.0).field2
    }

    let mut srs = SRS::<Test, TestRef>::create_with(
        Test {
            field: 2,
            field2: 2,
        },
        |owner| TestRef(owner),
    );
    // let r = srs.get_ref(|user,_|user);

    let b = test(&srs);
    let mut ow = Box::new(Test {
        field: 0,
        field2: 0,
    });
    let r = srs.split(&mut ow);
    assert_eq!(2, r.0.field);
    assert_eq!(2, b);
}

#[test]
fn test_new_and_get_ref() {
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
fn test_string_suffix_array() {
    struct TestRef<'a>(Vec<&'a str>);
    deref_with_lifetime!(TestRef);

    let mut suffix_array =
        SRS::<_, TestRef>::create_with("testtest こんにちは".to_owned(), |owner| {
            TestRef({
                let mut a = owner.chars();
                let mut vec = Vec::new();
                vec.push(a.as_str());
                while let Some(_) = a.next() {
                    vec.push(a.as_str());
                }
                vec.sort();
                vec
            })
        });
    //move it wherever you like
    fn contains(vec: &[&str], str: &str) -> bool {
        vec.binary_search_by(|&it| {
            if it.starts_with(str) {
                Ordering::Equal
            } else {
                it.cmp(str)
            }
        }).is_ok()
    }
    let a = suffix_array.with(|user, _| contains(&user.0, "ttes"));
    let c = suffix_array.with(|user, _| contains(&user.0, "こん"));
    let str = "aa".to_owned();
    let b = suffix_array.with(move |user, _| contains(&user.0, &*str));
    assert!(a && c && !b);
}

#[test]
fn test_cell() {
    struct TestRef<'a>(&'a Cell<u8>);
    deref_with_lifetime!(TestRef);

    fn helper(srs: SRS<Cell<u8>, TestRef<'static>>) -> u8 {
        srs.set(10);
        srs.get_ref(|user, _| user.0).set(20);
        srs.deref().get()
    }

    let mut srs = SRS::<_, TestRef<'static>>::create_with(Cell::new(25), |owner| TestRef(owner));
    let res = helper(srs);
    assert_eq!(res, 20);
}

#[test]
fn test_cell_raw_ref() {
    fn helper(srs: SRS<Cell<u8>, &'static Cell<u8>>) -> u8 {
        srs.set(10);
        srs.get_ref(|user, _| *user).set(20);
        srs.deref().get()
    }

    let mut srs = SRS::<_, &'static Cell<u8>>::create_with(Cell::new(25), |owner| owner);
    let res = helper(srs);
    assert_eq!(res, 20);
}

// this should never be able to compile
// todo check this with trybuild crate
// #[test]
// fn test_cell_user_ref() {
//     #[derive(Default)]
//     struct User<'a>(Cell<Option<&'a User<'a>>>);
//     deref_with_lifetime!(User);
//     // fn helper(srs: Box<SRS<(),User>>)  {
//     //     srs.get(|user|println!("{:p}",user.0.get().unwrap() as *const _))
//     // }
//
//     let mut srs = SRS::<(), User>::default();
//     let mut srs2 = SRS::<(), User>::default();
//     srs.get(|user|{
//         user.0.set(Some(user))
//     });
//     let before = srs.get(|user|user.0.get().unwrap() as *const _ as usize);
//     core::mem::swap(&mut srs,&mut srs2);
//     let after = srs.get(|user|user.0.get().unwrap() as *const _ as usize);
//     assert_eq!(before,after);
// }

#[rustversion::since(1.36)]
mod arena {
    use typed_arena::Arena;
    use gsrs::*;
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
        srs.with(|data, arena| {
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
}
