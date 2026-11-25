/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::rc::Rc;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::MutableHandleValue;
use rustc_hash::FxBuildHasher;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::DataTransferItemListBinding::DataTransferItemListMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::frozenarray::CachedFrozenArray;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::HashMapTracedValues;
use crate::dom::datatransferitem::DataTransferItem;
use crate::dom::file::File;
use crate::dom::filelist::FileList;
use crate::dom::window::Window;
use crate::drag::drag_data_store::{DragDataStore, Kind, Mode};

#[dom_struct]
pub(crate) struct DataTransferItemList {
    reflector_: Reflector,
    #[conditional_malloc_size_of]
    #[no_trace]
    data_store: Rc<RefCell<Option<DragDataStore>>>,
    #[ignore_malloc_size_of = "mozjs"]
    frozen_types: CachedFrozenArray,
    /// The File objects that represent the File items, by item id, so that the same File
    /// object is handed out for an item each time (and the one that was added is handed back),
    /// as Gecko's DataTransfer does.
    files: DomRefCell<HashMapTracedValues<u16, Dom<File>, FxBuildHasher>>,
    file_list: MutNullableDom<FileList>,
}

impl DataTransferItemList {
    fn new_inherited(data_store: Rc<RefCell<Option<DragDataStore>>>) -> DataTransferItemList {
        DataTransferItemList {
            reflector_: Reflector::new(),
            frozen_types: CachedFrozenArray::new(),
            data_store,
            files: DomRefCell::new(HashMapTracedValues::new_fx()),
            file_list: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        data_store: Rc<RefCell<Option<DragDataStore>>>,
    ) -> DomRoot<DataTransferItemList> {
        reflect_dom_object(
            cx,
            Box::new(DataTransferItemList::new_inherited(data_store)),
            window,
        )
    }

    /// The File object for the File item with the given id.
    pub(crate) fn file_for_item(&self, cx: &mut JSContext, id: u16) -> Option<DomRoot<File>> {
        if let Some(file) = self.files.borrow().get(&id) {
            return Some(DomRoot::from_ref(&**file));
        }
        let file = self
            .data_store
            .borrow()
            .as_ref()?
            .get_by_id(&id)?
            .as_file(cx, &self.global())?;
        self.files
            .safe_borrow_mut(cx.no_gc())
            .insert(id, Dom::from_ref(&*file));
        Some(file)
    }

    pub(crate) fn frozen_types(&self, cx: &mut js::context::JSContext, retval: MutableHandleValue) {
        self.frozen_types.get_or_init(
            cx,
            || {
                self.data_store
                    .borrow()
                    .as_ref()
                    .map_or(Vec::new(), |data_store| data_store.types())
            },
            retval,
        );
    }

    fn visible_files(&self, cx: &mut JSContext) -> Vec<DomRoot<File>> {
        let ids = self
            .data_store
            .borrow()
            .as_ref()
            .map_or_else(Vec::new, DragDataStore::file_item_ids);
        ids.into_iter()
            .filter_map(|id| self.file_for_item(cx, id))
            .collect()
    }

    pub(crate) fn file_list(&self, cx: &mut JSContext) -> DomRoot<FileList> {
        self.file_list.or_init(|| {
            let files = self.visible_files(cx);
            FileList::new(cx, self.global().as_window(), files)
        })
    }

    pub(crate) fn refresh_file_list(&self, cx: &mut JSContext) {
        if let Some(list) = self.file_list.get() {
            let files = self.visible_files(cx);
            list.replace_files(cx.no_gc(), &files);
        }
    }

    pub(crate) fn invalidate_frozen_types(&self) {
        self.frozen_types.clear();
    }
}

impl DataTransferItemListMethods<crate::DomTypeHolder> for DataTransferItemList {
    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-length>
    fn Length(&self) -> u32 {
        // Return zero if the object is in the disabled mode;
        // otherwise it must return the number of items in the drag data store item list.
        self.data_store
            .borrow()
            .as_ref()
            .map_or(0, |data_store| data_store.list_len() as u32)
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-item>
    fn IndexedGetter(
        &self,
        cx: &mut js::context::JSContext,
        index: u32,
    ) -> Option<DomRoot<DataTransferItem>> {
        // Step 1 Return null if it isn't associated with a data store
        let option = self.data_store.borrow();
        let data_store = match option.as_ref() {
            Some(value) => value,
            _ => return None,
        };

        // Step 2
        data_store.get_by_index(index as usize).map(|(id, _)| {
            DataTransferItem::new(cx, &self.global(), Rc::clone(&self.data_store), *id)
        })
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-add>
    fn Add(
        &self,
        cx: &mut js::context::JSContext,
        data: DOMString,
        mut type_: DOMString,
    ) -> Fallible<Option<DomRoot<DataTransferItem>>> {
        // Step 1 If the DataTransferItemList object is not in the read/write mode, return null.
        let index = {
            let mut option = self.data_store.borrow_mut();
            let data_store = match option.as_mut() {
                Some(value) if value.mode() == Mode::ReadWrite => value,
                _ => return Ok(None),
            };

            // Add an item to the drag data store item list whose kind is text,
            // whose type string is equal to the value of the method's second argument, converted to ASCII lowercase,
            // and whose data is the string given by the method's first argument.
            type_.make_ascii_lowercase();
            data_store.add(Kind::Text { data, type_ })
        };

        index.map(|id| {
            self.frozen_types.clear();

            // Step 3 Determine the value of the indexed property corresponding to the newly added item,
            // and return that value (a newly created DataTransferItem object).
            Some(DataTransferItem::new(
                cx,
                &self.global(),
                Rc::clone(&self.data_store),
                id,
            ))
        })
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-add>
    fn Add_(
        &self,
        cx: &mut js::context::JSContext,
        data: &File,
    ) -> Fallible<Option<DomRoot<DataTransferItem>>> {
        let index = {
            // Step 1 If the DataTransferItemList object is not in the read/write mode, return null.
            let mut option = self.data_store.borrow_mut();
            let data_store = match option.as_mut() {
                Some(value) if value.mode() == Mode::ReadWrite => value,
                _ => return Ok(None),
            };

            // Add an item to the drag data store item list whose kind is File,
            // whose type string is the type of the File, converted to ASCII lowercase,
            // and whose data is the same as the File's data.
            let mut type_ = data.file_type();
            type_.make_ascii_lowercase();
            let bytes = data.file_bytes().unwrap_or_default();
            let name = data.name().clone().root();
            data_store.add(Kind::File { bytes, name, type_ })
        };
        if let Ok(id) = index {
            self.files
                .safe_borrow_mut(cx.no_gc())
                .insert(id, Dom::from_ref(data));
            self.refresh_file_list(cx);
        }
        index.map(|id| {
            self.frozen_types.clear();

            // Step 3 Determine the value of the indexed property corresponding to the newly added item,
            // and return that value (a newly created DataTransferItem object).
            Some(DataTransferItem::new(
                cx,
                &self.global(),
                Rc::clone(&self.data_store),
                id,
            ))
        })
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-remove>
    fn Remove(&self, cx: &mut JSContext, index: u32) -> Fallible<()> {
        // Step 1 If the DataTransferItemList object is not in the read/write mode,
        // throw an "InvalidStateError" DOMException.
        let id = {
            let mut option = self.data_store.borrow_mut();
            let data_store = match option.as_mut() {
                Some(store) if store.mode() == Mode::ReadWrite => store,
                _ => {
                    return Err(Error::InvalidState(Some(
                        "The drag data store is not writable.".into(),
                    )));
                },
            };
            let index = index as usize;
            let Some((&id, _)) = data_store.get_by_index(index) else {
                return Ok(());
            };
            data_store.remove(index);
            id
        };
        self.files.safe_borrow_mut(cx.no_gc()).remove(&id);
        self.frozen_types.clear();
        self.refresh_file_list(cx);
        Ok(())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-datatransferitemlist-clear>
    fn Clear(&self, cx: &mut JSContext) {
        // If the DataTransferItemList object is in the read/write mode, remove all the items from the drag data store.
        // Otherwise, it must do nothing.
        {
            let mut option = self.data_store.borrow_mut();
            let data_store = match option.as_mut() {
                Some(store) if store.mode() == Mode::ReadWrite => store,
                _ => return,
            };
            if data_store.list_len() == 0 {
                return;
            }
            data_store.clear_list();
        }
        *self.files.safe_borrow_mut(cx.no_gc()) = HashMapTracedValues::new_fx();
        self.frozen_types.clear();
        self.refresh_file_list(cx);
    }
}
