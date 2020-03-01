//! # GSRS or Generic Self Referencing Struct
//!
//! This crate helps to create custom movable self referencing structs.
//! Nothing magical. It just wraps Owner and references to it in single package
//! with simple but unsafe lifetime tricks.
//!
//! Self referencing structs are generally considered an anti-pattern in Rust, so if you can easily
//! go without it you should do it. But sometimes you actually need to have a self referential struct.
//! So here are some examples when you actually need `SRS`:
//!  - If you have structure that is built on references
//! (graph with Arena, or any structure built with slices on top of the string)
//! and you want to be able move it to another thread, or put it into Vec.
//!  - If your api would be much better if you will be able to return self contained values.
//!
//! Does not support dependent lifetimes (yet?, is it actully needed/possible?)
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
//! or you can do creation in one go.
//! Although you anyway have to specify type of the referencing part
//! because type inference is getting confused by changed lifetime.
//! This can be fixed, but only when GATs will be stabilized.
//! ```
//! use gsrs::*;
//! struct Test{field:usize}
//! struct TestRef<'a>(&'a Test);
//! deref_with_lifetime!(TestRef);
//! // create owned part and self-referencing part
//! let mut srs = SRS::<_, TestRef>::create_with(
//!     Test{ field: 5 },
//!     |owner|TestRef(owner)
//! );
//! // get self referencing part back
//! let r = srs.get_ref(|user, _| user.0);
//! println!("{}", r.field);
//! ```
//! Referencing part can be arbitrary complex:
//! ```
//! use gsrs::*;
//! struct TestRef<'a>(Vec<&'a str>);
//! deref_with_lifetime!(TestRef);
//! // create owned part and self-referencing part
//! let mut srs = SRS::<_, TestRef>::create_with(
//!     "long unicode string".to_owned(),
//!     |owner|TestRef(owner.split(' ').collect())
//! );
//! // get self referencing part back
//! let r = srs.get_ref(|user, _| user.0[1]);
//! assert_eq!("unicode", r);
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
use std::ptr::NonNull;
use std::fmt::{Debug, Formatter};
// use std::marker::PhantomPinned;
// use std::pin::Pin;

// pub unsafe trait Movable:Unpin{}
// unsafe impl<T:Unpin> Movable for Box<T>{}
// unsafe impl<T:Unpin> Movable for Arena<T>{}
// unsafe impl<T:Unpin> Movable for Vec<T>{}
/// ## Self Referencing Struct
/// Allows owner and references to it to be saved in a same movable struct
///
/// In general you create `SRS` with `create_with`, modify it with `with`, use it with `get_ref`
/// and in the end it will be dropped automatically or you can use `split` to keep some parts if necessary.f
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
    // user: AliasedBox<U>,
    user: U,
    // Box is required to prevent user to get reference to owner field, because it would be invalid after move
    // so it would be possible to move SRS safely
    // Technically i think it can also be done by providing some king of collection trait but
    // it is a todo right now
    // We need to AliasedBox instead usual Box because we violate noalias Box requirement
    // With Box when SRS is moved into function, compiler/llvm expects that there is no other pointers
    // pointing inside of it, so it can discard any action that is using reference from U
    owner: AliasedBox<Owner>,
}

// uncomment if U is UnsafeCell
// unsafe impl<Owner,U> Sync for SRS<Owner,U>
//     where
//         U: for<'b> DerefWithLifetime<'b>+Sync,
//         Owner: Sync
// {}

impl<Owner: Default, U: Default> Default for SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    fn default() -> Self {
        Self {
            owner: Box::new(<Owner as Default>::default()).into(),
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
            owner: Box::new(owner).into(),
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
        F: 'static + FnOnce(&'b Owner) -> <U as DerefWithLifetime<'b>>::Target,
        Owner: 'b,
        U: 'b,
    {
        let owner: AliasedBox<Owner> = Box::new(owner).into();

        let owner_ref = owner.deref();
        let user = unsafe {
            // transmute here also just changes lifetime
            <U as DerefWithLifetime>::move_with_lifetime_back(f(transmute(owner_ref)))
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
        let owner = unsafe { &mut *(&mut self.owner as *mut _ as *mut Box<Owner>) };
        mem::swap(new, owner);
        unsafe { self.user.move_with_lifetime() }
    }

    /// ### Main interface to modify `SRS`
    /// Used to actually create or mutate SRS
    ///
    /// ### Safety
    /// `'static` lifetime on closure and on return value is required to prevent saving outer references in `user`
    /// and enforcing `'b` lifetime allows to use references to data inside this struct outside.
    /// Moving struct is safe because you can't get reference to the underlying fields
    /// (`Owner` is behind `Box` and `U` is behind incompatible lifetime when passed into closure).
    #[inline]
    pub fn with<'b, F, Z: 'static>(&'b mut self, f: F) -> Z
    where
        for<'x> F: 'static + FnOnce(&'x mut <U as DerefWithLifetime<'b>>::Target, &'b Owner) -> Z,
        'a: 'b,
    {
        let owner = self.owner.deref();
        let user = unsafe { self.user.deref_with_lifetime_mut() };
        f(user, owner)
    }

    /// ### Method for using 'SRS'
    /// Allows you to get existing self reference to use it outside
    ///
    /// ### Safety
    /// Same as for `with`
    #[inline]
    pub fn get_ref<'b, F, Z: ?Sized + 'static>(&'b self, f: F) -> &'b Z
    where
        for<'x> F: 'static + FnOnce(&'x <U as DerefWithLifetime<'b>>::Target, &'b Owner) -> &'b Z,
        'a: 'b,
    {
        let owner = self.owner.deref();
        let user = unsafe { self.user.deref_with_lifetime() };
        f(user, owner)
    }

    // pub fn get<'b, F, Z: 'static>(&'b self, f: F) -> Z
    //     where
    //         for <'x> F: 'static + FnOnce(&'x <U as DerefWithLifetime<'b>>::Target) -> Z,
    //         'a: 'b,
    // {
    //     let user = unsafe { self.user.deref_with_lifetime() };
    //     f(user)
    // }
}

impl<'a, Owner: 'a, U> Deref for SRS<Owner, U>
where
    U: for<'b> DerefWithLifetime<'b>,
{
    type Target = Owner;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.owner.deref()
    }
}

// technically default drop is safe for current rust version
// but manually implementing drop is more future proof
// in case rust will allow to run particular code only if lifetime is static
// because in that case malicious drop impls will be able to save inner references in outer static variables
// impl<'a, Owner: 'a, U> Drop for SRS<Owner, U>
//     where
//         U: for<'b> DerefWithLifetime<'b>,
// {
//     fn drop<'a>(&'a mut self) {
//         unsafe {
//             drop_in_place(<U as DerefWithLifetime<'a>>::deref_with_lifetime_mut(&mut self.user));
//             drop_in_place(&mut self.owner)
//         }
//     }
// }

struct AliasedBox<U: ?Sized> {
    ptr: NonNull<U>,
}

impl<U: Default + ?Sized> Default for AliasedBox<U> {
    fn default() -> Self {
        Box::new(U::default()).into()
    }
}

impl<U: Debug> Debug for AliasedBox<U> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

impl<U: ?Sized> Deref for AliasedBox<U> {
    type Target = U;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr.as_ref() as *const _ as *const Self::Target) }
        // unsafe { self.ptr.as_ref() }
    }
}

// impl<U: ?Sized> AliasedBox<U>{
//     fn into(self) -> Box<U> {
//         unsafe {
//             let ptr = self.ptr.as_ptr();
//             mem::forget(self);
//             Box::from_raw(ptr)
//         }
//     }
// }

impl<U: ?Sized> From<Box<U>> for AliasedBox<U> {
    #[inline]
    fn from(from: Box<U>) -> Self {
        unsafe {
            AliasedBox {
                ptr: NonNull::new_unchecked(Box::into_raw(from) as *mut _),
            }
        }
    }
}

impl<U: ?Sized> Drop for AliasedBox<U> {
    fn drop(&mut self) {
        unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

// /// This one is the most efficient but most restrictive.
// ///
// ///
// #[derive(Debug)]
// pub struct SRSThin<U1, U2>
// where
//     U1: for<'b> DerefWithLifetime<'b>,
//     U2: for<'b> DerefWithLifetime<'b>,
// {
//     user1: U1,
//     user2: U2,
//     pinned: PhantomPinned,
// }
//
// impl<U1, U2> SRSThin<U1, U2>
// where
//     U1: for<'b> DerefWithLifetime<'b>,
//     U2: for<'b> DerefWithLifetime<'b>,
// {
//
//     pub fn new(part1: U1, part2: U2) -> Self {
//         Self{
//             user1: part1,
//             user2: part2,
//             pinned: PhantomPinned
//         }
//     }
// }

/// This trait should be implemented for any struct that will contain references to data inside `SRS`
/// and it should be implemented for any lifetime.
/// Basically it just allows to apply custom lifetime to struct
///
/// It is already implemented for pure references.
/// In general `deref_with_lifetime' macro should be used to implement this trait safely.
///
/// It is unsafe because SRS expects implementations of this trait to only change lifetime.
///
/// TODO this will only be implemented with macro in future
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

unsafe impl<'a, Z: ?Sized + 'static> DerefWithLifetime<'a> for &'_ Z {
    type Target = &'a Z;

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
            #[inline(always)]
            unsafe fn deref_with_lifetime(&'a self) -> &'a Self::Target {
                core::mem::transmute(self)
            }

            #[inline(always)]
            unsafe fn deref_with_lifetime_mut(&'a mut self) -> &'a mut Self::Target {
                core::mem::transmute(self)
            }

            #[inline(always)]
            unsafe fn move_with_lifetime(self) -> Self::Target {
                core::mem::transmute(self)
            }

            #[inline(always)]
            unsafe fn move_with_lifetime_back(this: Self::Target) -> Self {
                core::mem::transmute(this)
            }
        }
    };
}
