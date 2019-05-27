#[macro_use]
extern crate log;
#[macro_use]
extern crate mruby;

use mruby::convert::TryFromMrb;
use mruby::def::{rust_data_free, ClassLike, Define};
use mruby::eval::MrbEval;
use mruby::file::MrbFile;
use mruby::interpreter::{Interpreter, Mrb};
use mruby::load::MrbLoadSources;
use mruby::sys;
use mruby::value::Value;
use mruby::MrbError;
use std::cell::RefCell;
use std::ffi::{c_void, CString};
use std::mem;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
struct Container {
    inner: i64,
}

impl Container {
    extern "C" fn initialize(mrb: *mut sys::mrb_state, mut slf: sys::mrb_value) -> sys::mrb_value {
        unsafe {
            let interp = interpreter_or_raise!(mrb);

            let int = mem::uninitialized::<sys::mrb_int>();
            let argspec = CString::new(sys::specifiers::INTEGER).expect("argspec");
            sys::mrb_get_args(mrb, argspec.as_ptr(), &int);
            let cont = Container { inner: int };
            let data = Rc::new(RefCell::new(cont));
            debug!("Storing `Container` refcell in self instance: {:?}", data);
            let ptr = mem::transmute::<Rc<RefCell<Container>>, *mut c_void>(data);

            debug!(
                "interpreter strong ref count = {}",
                Rc::strong_count(&interp)
            );
            let spec = class_spec_or_raise!(interp, Container);
            sys::mrb_sys_data_init(&mut slf, ptr, spec.borrow().data_type());

            slf
        }
    }

    extern "C" fn value(mrb: *mut sys::mrb_state, slf: sys::mrb_value) -> sys::mrb_value {
        unsafe {
            let interp = interpreter_or_raise!(mrb);
            let spec = class_spec_or_raise!(interp, Container);

            debug!("pulled mrb_data_type from user data with class: {:?}", spec);
            let borrow = spec.borrow();
            let ptr = sys::mrb_data_get_ptr(mrb, slf, borrow.data_type());
            let data = mem::transmute::<*mut c_void, Rc<RefCell<Container>>>(ptr);
            let clone = Rc::clone(&data);
            let cont = clone.borrow();

            let value = unwrap_value_or_raise!(interp, Value::try_from_mrb(&interp, cont.inner));
            mem::forget(data);
            value
        }
    }
}

impl MrbFile for Container {
    fn require(interp: Mrb) -> Result<(), MrbError> {
        let spec = {
            let mut api = interp.borrow_mut();
            let spec = api.def_class::<Self>("Container", None, Some(rust_data_free::<Self>));
            spec.borrow_mut()
                .add_method("initialize", Self::initialize, sys::mrb_args_req(1));
            spec.borrow_mut()
                .add_method("value", Self::value, sys::mrb_args_none());
            spec.borrow_mut().mrb_value_is_rust_backed(true);
            spec
        };
        spec.borrow().define(&interp)?;
        Ok(())
    }
}

#[test]
fn define_rust_backed_ruby_class() {
    env_logger::Builder::from_env("MRUBY_LOG").init();

    let interp = Interpreter::create().expect("mrb init");
    interp
        .def_file_for_type::<_, Container>("container")
        .expect("def file");

    let code = "require 'container'; Container.new(15).value";
    let result = interp.eval(code).expect("no exceptions");
    let cint = unsafe { i64::try_from_mrb(&interp, result).expect("convert") };
    assert_eq!(cint, 15);

    drop(interp);
}
