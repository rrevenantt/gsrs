//! # GSRS or Generic Self Referencing Struct
//!
//! This crate helps to create custom movable self referencing structs.
//! And with just very little unsafe. No raw pointer magic and basically just a signle unsafe lifetime trick.
//! Although if you want to extend your struct dynamically some more unsafe is required.
//!
//! Self referencing structs are generally considered an anti-pattern in Rust, so if you can easily
//! go without it you should do it. But sometimes you actually need to have a self referential struct.
//! So here are some examples when you actually need `SRS`:
//!  - You have a structs with `Vec` of big values and a `HashSet` to prevent adding duplicates to it.
//! You also need to dynamically add values to it from time to time. Also those structs are holded in another collection.
//! Technically you might be able to workaround with indicies, or untrivially reorganising your code, but it would
//! require additional code and makes semantics less clear.
//!
//! Does not support dependent lifetimes.(is it actully needed/possible?)
//!
//! Should work on any stable rust starting from 1.31(2018 edition)
//!
//! # Usage
//! Simple example:
//! ```
//! use gsrs::*;
//! struct Test{field:usize}
//! #[derive(Default)]
//! struct TestRef<'a>(Option<&'a Test>);
//! deref_with_lifetime!(TestRef);
//! // create owned part
//! let mut srs = SRS::<Test, TestRef>::new( Test{ field: 5 } );
//! // create self-referencing part
//! srs.with(|user, owner|*user = TestRef(Some(owner)));
//! // get self referencing part back
//! let r = srs.get_ref(|user, _| user.0.unwrap());
//! println!("{}", r.field);
//! ```
//! or you can do creation in one go
//! ```
//! use gsrs::*;
//! struct Test{field:usize}
//! struct TestRef<'a>(&'a Test);
//! deref_with_lifetime!(TestRef);
//! // create owned part and self-referencing part
//! let mut srs = SRS::<Test, TestRef>::create_with(
//!     Test{ field: 5 },
//!     |owner|TestRef(owner)
//! );
//! // get self referencing part back
//! let r = srs.get_ref(|user, _| user.0);
//! println!("{}", r.field);
//! ```
//!
//! and this won't compile because `get_ref` is able to supply return value with proper lifetime:
//! ```compile_fail
//! # use gsrs::*;
//! # struct Test{field:usize}
//! # #[derive(Default)]
//! # struct TestRef<'a>(Option<&'a Test>);
//! # deref_with_lifetime!(TestRef);
//! # let mut srs = SRS::<Test,TestRef<'static>>::new(Test{field:5});
//! srs.with(|user, owner|*user = TestRef(Some(owner)));
//! let r = srs.get_ref(|user, _| user.0.unwrap());
//! drop(srs);
//! println!("{}",r.field);
//! ```
//! This also will fail because it is possible to return only static types or references to static types.
//! It is done to prevent changing some inner reference with interior mutability.
//! ```compile_fail
//! # use gsrs::*;
//! # struct Test{field:usize}
//! # struct TestRef<'a>(&'a Test);
//! # deref_with_lifetime!(TestRef);
//! let mut srs = SRS::<Test,TestRef<'static>>::create_with(
//!     Test{field:5},
//!     |owner|TestRef(owner),
//! );
//! // here closure returns TestRef<'a> not a reference
//! let r = srs.with(|user,_|user);
//! let mut ow = Box::new(Test{field:0});
//! let r = srs.split(&mut ow);
//! println!("{}",r.0.field);
//! ```
#![warn(missing_docs)]
// use std::intrinsics::transmute;

// pub unsafe trait ExtendedWhileBorrowed:Movable {}

use std::ops::Deref;
use std::mem;
use std::intrinsics::transmute;

// pub unsafe trait Movable:Unpin{}
// unsafe impl<T:Unpin> Movable for Box<T>{}
// unsafe impl<T:Unpin> Movable for Arena<T>{}
// unsafe impl<T:Unpin> Movable for Vec<T>{}
/// ## Self Referencing Struct
/// Allows owner and references to it to be saved in a same movable struct
///
/// In general you create `SRS` with `create_with`, modify it with `with`, use it with `get_ref`
/// and in the end it will be dropped automatically or you can use `split` to keep some parts if necessary
///
/// If you want to add additional owned values you will need arena-like structure like Arena from `typed_arena`
///
/// If `Owner` type can be extended while there are references to existing data, like Arena,
/// you can use `default` otherwise `new` is the only way to create it
///
/// It is recommended to annotate lifetime used for `DerefWithLifetime` impl as `'static` when creating `SRS`
/// otherwise it might be impossible to move it.
#[derive(Debug)]
pub struct SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    // user have to be before owner for proper Drop call order
    user: U,
    // Box is required to prevent user to get reference to owner field, because it would be invalid after move
    // so it would be possible to move SRS safely
    // Technically i think it can also be done by providing some king of collection trait but
    // it is a todo right now
    owner: Box<Owner>,
}

impl<Owner: Default, U: Default> Default for SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    fn default() -> Self {
        Self {
            owner: Default::default(),
            user: Default::default(),
        }
    }
}

impl<'a, Owner: 'a, U: Default> SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    /// Creates new SRS instance without any actual self reference.
    /// `with` method should be used to add self references afterwards
    pub fn new(owner: Owner) -> Self {
        Self {
            owner: Box::new(owner),
            user: Default::default(),
        }
    }
}

// pub trait TypeEquals {
//     type Other;
//     fn into_self(self) -> Self::Other;
// }
//
// impl<'b, T: DerefWithLifetime<'b>> TypeEquals for T {
//     type Other = Self;
//
//     fn into_self(self) -> Self::Other {
//         self
//     }
// }

impl<'a, Owner: 'a, U> SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    /// Creates `SRS` from `Owner` and a function that creates self referencing part from owner
    ///
    /// ```
    /// use gsrs::*;
    /// struct Test{field:usize}
    /// struct TestRef<'a>(&'a Test);
    /// deref_with_lifetime!(TestRef);
    /// // let a = None;
    /// let mut srs = SRS::<Test,TestRef<'static>>::create_with(
    ///     Test{field:5},
    ///     |owner|TestRef(owner),
    /// );
    /// let r = srs.get_ref(|user,_|user.0);
    /// let mut ow = Box::new(Test{field:0});
    /// let r = srs.split(&mut ow);
    /// println!("{}",r.0.field);
    /// ```
    #[inline]
    pub fn create_with<'b, F>(owner: Owner, f: F) -> Self
    where
        // for<'b> Z: DerefWithLifetime<'b,Static=U>,
        F: FnOnce(&'b Owner) -> <U as DerefWithLifetime<'b>>::Target + 'static,
        Owner: 'b,
        U: 'b, // for<'b> <U as DerefWithLifetime<'b>>
    {
        let owner = Box::new(owner);

        let user = {
            let owner_ref = owner.as_ref();
            let v = unsafe {
                // transmute here also just changes lifetime
                <U as DerefWithLifetime>::move_with_lifetime_back(f(transmute(owner_ref)))
            };
            v
        };

        Self { owner, user }
    }

    /// Splits `SRS` into owned and borrowed parts.
    ///
    /// Be careful because reverse operation is impossible because there is no way to know that references,
    /// that we will bundle with `Owner`, are actually all pointing inside `Owner`.
    ///
    /// It requires some existing `Owner` because it needs place where to move it out and get lifetime from.
    /// ```
    /// use gsrs::*;
    /// struct Test{field:usize}
    /// #[derive(Default)]
    /// struct TestRef<'a>(Option<&'a Test>);
    /// deref_with_lifetime!(TestRef);
    /// let mut srs = SRS::<Test,TestRef<'static>>::new(Test{field:5});
    /// srs.with(|user, owner|*user = TestRef(Some(owner)));
    /// // do some work with srs
    /// let mut ow = Box::new(Test{field:0});
    /// let r = srs.split(&mut ow);
    /// println!("{}",r.0.unwrap().field);
    /// ```
    #[inline]
    pub fn split<'b>(mut self, new: &'b mut Box<Owner>) -> <U as DerefWithLifetime<'b>>::Target {
        mem::swap(new, &mut self.owner);
        unsafe { self.user.move_with_lifetime() }
    }

    /// ### Main interface to modify `SRS`
    /// Used to actually create or mutate SRS
    ///
    /// ### Safety
    /// `'static` lifetime on closure and on return value is required to prevent saving outer references in `user`
    /// and enforcing `'b` lifetime allows to use references to data inside this struct outside.
    /// Moving struct is safe because you can't get reference to the fields.
    #[inline]
    pub fn with<'b, F, Z: 'static>(&'b mut self, f: F) -> Z
    where
        F: FnOnce(&'b mut <U as DerefWithLifetime<'b>>::Target, &'b Owner) -> Z + 'static,
        'a: 'b,
    {
        let arena = self.owner.as_ref();
        let user = unsafe { self.user.deref_with_lifetime_mut() };
        f(user, arena)
    }

    /// ### Method for using 'SRS'
    /// Allows you to get existing self reference
    ///
    /// ### Safety
    /// Same as for `with`
    #[inline]
    pub fn get_ref<'b, F, Z: 'static>(&'b self, f: F) -> &'b Z
    where
        F: FnOnce(&'b <U as DerefWithLifetime<'b>>::Target, &'b Owner) -> &'b Z + 'static,
        'a: 'b,
    {
        let arena = self.owner.as_ref();
        let user = unsafe { self.user.deref_with_lifetime() };
        f(user, arena)
    }
}

impl<'a, Owner: 'a, U: Default> Deref for SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    type Target = Owner;

    fn deref(&self) -> &Self::Target {
        self.owner.as_ref()
    }
}

/// This trait should be implemented for any struct that will contain references to data inside `SRS`
/// and it should be implemented for any lifetime.
/// Basically it just allows to apply custom lifetime to struct
///
/// For simple cases it is better to use `deref_with_lifetime' macro.
///
/// It can not introduce any unsoundness by itself, because this functions are unsafe,
/// so it is their caller responsibility to be sound
///
/// TODO this will only be implemented with macro in future
/// TODO not sure if trait have to be unsafe itself
pub unsafe trait DerefWithLifetime<'a> {
    /// Implementors should make `Target` a Self but generic over `'a`, see macro definition
    type Target: 'a;
    // type Static: 'static;
    /// implementation should be just `transmute(self)` to only change lifetime
    unsafe fn deref_with_lifetime(&'a self) -> &'a Self::Target;

    /// implementation should be just `transmute(self)` to only change lifetime
    unsafe fn deref_with_lifetime_mut(&'a mut self) -> &'a mut Self::Target;

    /// implementation should be just `transmute(self)` to only change lifetime
    unsafe fn move_with_lifetime(self) -> Self::Target;

    /// implementation should be just `transmute(self)` to only change lifetime
    unsafe fn move_with_lifetime_back(this: Self::Target) -> Self;

    // unsafe fn move_as_static(self) -> Self::Static;
}

/// Macro to implement `DerefWithLifetime`
///
/// Currently only works for simple cases with one lifetime and no generic,
/// but in future this will be the only way to implement trait
#[macro_export]
macro_rules! deref_with_lifetime {
    ($struct: tt) => {
        unsafe impl<'a> DerefWithLifetime<'a> for $struct<'_> {
            type Target = $struct<'a>;
            // type Static = $struct<'static>;
            unsafe fn deref_with_lifetime(&'a self) -> &'a Self::Target {
                core::mem::transmute(self)
            }

            unsafe fn deref_with_lifetime_mut(&'a mut self) -> &'a mut Self::Target {
                core::mem::transmute(self)
            }

            unsafe fn move_with_lifetime(self) -> Self::Target {
                core::mem::transmute(self)
            }

            unsafe fn move_with_lifetime_back(this: Self::Target) -> Self {
                core::mem::transmute(this)
            }

            // unsafe fn move_as_static(self) -> Self::Static {
            //     core::mem::transmute(self)
            // }
        }
    };
}
