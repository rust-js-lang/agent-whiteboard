//! idb@8.0.4's promise API, specialized by callers to our string-keyed JSON store.
use std::marker::PhantomData;

use js::{JsObject, Promise};
use webapi::IDBVersionChangeEvent;

#[cfg_attr(rust_js, rust_js::types = "idb#IDBPDatabase")]
pub struct Database(PhantomData<JsObject>);
#[cfg_attr(rust_js, rust_js::types = "idb#IDBPTransaction")]
pub struct Transaction(PhantomData<JsObject>);
#[cfg_attr(rust_js, rust_js::types = "idb#IDBPObjectStore")]
pub struct Store(PhantomData<JsObject>);

pub struct OpenOptions<F> {
    pub upgrade: F,
}
pub struct TransactionOptions {
    pub durability: &'static str,
}

#[cfg_attr(rust_js, rust_js::link_name = "idb#openDB")]
pub fn open_db<F>(name: &str, version: u32, options: OpenOptions<F>) -> Promise<&'static Database>
where
    F: Fn(
            &'static Database,
            f64,
            Option<f64>,
            &'static Transaction,
            &'static IDBVersionChangeEvent,
        ) + 'static,
{
    unreachable!()
}

impl Database {
    #[cfg_attr(rust_js, rust_js::link_name = "createObjectStore")]
    pub fn create_object_store(&self, name: &str) -> &'static Store {
        unreachable!()
    }
    #[cfg_attr(rust_js, rust_js::link_name = "transaction")]
    pub fn transaction(
        &self,
        store: &str,
        mode: &str,
        options: TransactionOptions,
    ) -> &'static Transaction {
        unreachable!()
    }
    #[cfg_attr(rust_js, rust_js::link_name = "close")]
    pub fn close(&self) {
        unreachable!()
    }
    #[cfg_attr(rust_js, rust_js::link_name = "set onversionchange")]
    pub fn set_on_version_change(&self, callback: impl Fn(&IDBVersionChangeEvent) + 'static) {
        unreachable!()
    }
}
impl Transaction {
    #[cfg_attr(rust_js, rust_js::link_name = "get done")]
    pub fn done(&self) -> Promise<()> {
        unreachable!()
    }
    #[cfg_attr(rust_js, rust_js::link_name = "objectStore")]
    pub fn object_store(&self, name: &str) -> &'static Store {
        unreachable!()
    }
}
impl Store {
    #[cfg_attr(rust_js, rust_js::link_name = "get")]
    pub fn get<T>(&self, key: &str) -> Promise<Option<T>> {
        unreachable!()
    }
    #[cfg_attr(rust_js, rust_js::link_name = "put")]
    pub fn put<T>(&self, value: T, key: &str) -> Promise<String> {
        unreachable!()
    }
}
