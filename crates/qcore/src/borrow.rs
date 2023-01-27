#[macro_export]
macro_rules! borrow {
    ($ty:ty { $field:ident : $field_ty:ty }) => {
        impl ::std::borrow::Borrow<$field_ty> for $ty {
            fn borrow(&self) -> &$field_ty { &self.$field }
        }
    };

    ($ty:ty { mut $field:ident : $field_ty:ty }) => {
        $crate::borrow!($ty { $field: $field_ty });

        impl ::std::borrow::BorrowMut<$field_ty> for $ty {
            fn borrow_mut(&mut self) -> &mut $field_ty { &mut self.$field }
        }
    };
}
