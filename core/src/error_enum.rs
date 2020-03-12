#[macro_export]
macro_rules! error_enum {
    {enum $enum_name:ident {} } => {
        pub enum $enum_name {}
    };
    {enum $enum_name:ident { $( $variant:ident ($type:ty) ),* }} => {
        error_enum! {
            enum $enum_name {
                $(
                    $variant($type),
                )*
            }
        }
    };
    {enum $enum_name:ident { $( $variant:ident ($type:ty) ),*, }} => {
        #[derive(Debug)]
        pub enum $enum_name {
            $(
                $variant($type),
            )*
        }
        $(
            impl From<$type> for $enum_name {
                fn from(err: $type) -> $enum_name {
                    $enum_name::$variant(err)
                }
            }
        )*
    };
}
