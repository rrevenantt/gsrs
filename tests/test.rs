use gsrs::deref_with_lifetime;
use gsrs::DerefWithLifetime;
use gsrs::SRS;

use typed_arena::Arena;

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
        let a = arena.alloc(MyBigStruct { f1: 0, _f2: None });
        data.type1.push(&*a);
        // &*a
    });

    // let b = srs.get_ref(|user,_| user.type1[0]);
    let b = test1(srs);

    println!("{}", b);
    // drop(srs);

    assert_eq!(2 + 2, 4);
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
        // srs.get_ref(|&user,_|&user).0.field
        0
    }

    let mut srs =
        SRS::<Test, TestRef<'static>>::create_with(Test { field: 5 }, |owner| TestRef(owner));
    // let r = srs.get_ref(|user,_|user);

    let b = test(&srs);
    let mut ow = Box::new(Test { field: 0 });
    let r = srs.split(&mut ow);
    println!("{}", r.0.field);
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
    let mut srs = SRS::<Test, TestRef>::new(Test { field: 5 });
    srs.with(|user, owner| *user = TestRef(Some(owner)));

    // let b = test(&srs);
    let r = srs.get_ref(|user, _| user.0.unwrap());
    println!("{}", r.field);
}
