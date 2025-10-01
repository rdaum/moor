// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.2.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `connections.fbs`
#[no_implicit_prelude]
#[allow(dead_code, clippy::needless_lifetimes)]
mod root {
    /// The namespace `MoorConnections`
    ///
    /// Generated from these locations:
    /// * File `connections.fbs`
    pub mod moor_connections {
        /// The table `ByteArray` in the namespace `MoorConnections`
        ///
        /// Generated from these locations:
        /// * Table `ByteArray` in the file `connections.fbs:16`
        #[derive(
            Clone,
            Debug,
            PartialEq,
            PartialOrd,
            Eq,
            Ord,
            Hash,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        pub struct ByteArray {
            /// The field `data` in the table `ByteArray`
            pub data: ::planus::alloc::vec::Vec<u8>,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for ByteArray {
            fn default() -> Self {
                Self {
                    data: ::core::default::Default::default(),
                }
            }
        }

        impl ByteArray {
            /// Creates a [ByteArrayBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> ByteArrayBuilder<()> {
                ByteArrayBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_data: impl ::planus::WriteAs<::planus::Offset<[u8]>>,
            ) -> ::planus::Offset<Self> {
                let prepared_data = field_data.prepare(builder);

                let mut table_writer: ::planus::table_writer::TableWriter<6> =
                    ::core::default::Default::default();
                table_writer.write_entry::<::planus::Offset<[u8]>>(0);

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        object_writer.write::<_, _, 4>(&prepared_data);
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<ByteArray>> for ByteArray {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ByteArray> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<ByteArray>> for ByteArray {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ByteArray>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<ByteArray> for ByteArray {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ByteArray> {
                ByteArray::create(builder, &self.data)
            }
        }

        /// Builder for serializing an instance of the [ByteArray] type.
        ///
        /// Can be created using the [ByteArray::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct ByteArrayBuilder<State>(State);

        impl ByteArrayBuilder<()> {
            /// Setter for the [`data` field](ByteArray#structfield.data).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn data<T0>(self, value: T0) -> ByteArrayBuilder<(T0,)>
            where
                T0: ::planus::WriteAs<::planus::Offset<[u8]>>,
            {
                ByteArrayBuilder((value,))
            }
        }

        impl<T0> ByteArrayBuilder<(T0,)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ByteArray].
            #[inline]
            pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<ByteArray>
            where
                Self: ::planus::WriteAsOffset<ByteArray>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[u8]>>>
            ::planus::WriteAs<::planus::Offset<ByteArray>> for ByteArrayBuilder<(T0,)>
        {
            type Prepared = ::planus::Offset<ByteArray>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ByteArray> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[u8]>>>
            ::planus::WriteAsOptional<::planus::Offset<ByteArray>> for ByteArrayBuilder<(T0,)>
        {
            type Prepared = ::planus::Offset<ByteArray>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ByteArray>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[u8]>>> ::planus::WriteAsOffset<ByteArray>
            for ByteArrayBuilder<(T0,)>
        {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ByteArray> {
                let (v0,) = &self.0;
                ByteArray::create(builder, v0)
            }
        }

        /// Reference to a deserialized [ByteArray].
        #[derive(Copy, Clone)]
        pub struct ByteArrayRef<'a>(::planus::table_reader::Table<'a>);

        impl<'a> ByteArrayRef<'a> {
            /// Getter for the [`data` field](ByteArray#structfield.data).
            #[inline]
            pub fn data(&self) -> ::planus::Result<&'a [u8]> {
                self.0.access_required(0, "ByteArray", "data")
            }
        }

        impl<'a> ::core::fmt::Debug for ByteArrayRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("ByteArrayRef");
                f.field("data", &self.data());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<ByteArrayRef<'a>> for ByteArray {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: ByteArrayRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    data: value.data()?.to_vec(),
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for ByteArrayRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for ByteArrayRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location(
                        "[ByteArrayRef]",
                        "get",
                        buffer.offset_from_start,
                    )
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<ByteArray>> for ByteArray {
            type Value = ::planus::Offset<ByteArray>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<ByteArray>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for ByteArrayRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[ByteArrayRef]", "read_as_root", 0)
                })
            }
        }

        /// The table `ClientAttribute` in the namespace `MoorConnections`
        ///
        /// Generated from these locations:
        /// * Table `ClientAttribute` in the file `connections.fbs:21`
        #[derive(
            Clone,
            Debug,
            PartialEq,
            PartialOrd,
            Eq,
            Ord,
            Hash,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        pub struct ClientAttribute {
            /// The field `key` in the table `ClientAttribute`
            pub key: ::planus::alloc::boxed::Box<self::ByteArray>,
            /// The field `value` in the table `ClientAttribute`
            pub value: ::planus::alloc::boxed::Box<self::ByteArray>,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for ClientAttribute {
            fn default() -> Self {
                Self {
                    key: ::core::default::Default::default(),
                    value: ::core::default::Default::default(),
                }
            }
        }

        impl ClientAttribute {
            /// Creates a [ClientAttributeBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> ClientAttributeBuilder<()> {
                ClientAttributeBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_key: impl ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
                field_value: impl ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            ) -> ::planus::Offset<Self> {
                let prepared_key = field_key.prepare(builder);
                let prepared_value = field_value.prepare(builder);

                let mut table_writer: ::planus::table_writer::TableWriter<8> =
                    ::core::default::Default::default();
                table_writer.write_entry::<::planus::Offset<self::ByteArray>>(0);
                table_writer.write_entry::<::planus::Offset<self::ByteArray>>(1);

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        object_writer.write::<_, _, 4>(&prepared_key);
                        object_writer.write::<_, _, 4>(&prepared_value);
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<ClientAttribute>> for ClientAttribute {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ClientAttribute> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<ClientAttribute>> for ClientAttribute {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ClientAttribute>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<ClientAttribute> for ClientAttribute {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ClientAttribute> {
                ClientAttribute::create(builder, &self.key, &self.value)
            }
        }

        /// Builder for serializing an instance of the [ClientAttribute] type.
        ///
        /// Can be created using the [ClientAttribute::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct ClientAttributeBuilder<State>(State);

        impl ClientAttributeBuilder<()> {
            /// Setter for the [`key` field](ClientAttribute#structfield.key).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn key<T0>(self, value: T0) -> ClientAttributeBuilder<(T0,)>
            where
                T0: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            {
                ClientAttributeBuilder((value,))
            }
        }

        impl<T0> ClientAttributeBuilder<(T0,)> {
            /// Setter for the [`value` field](ClientAttribute#structfield.value).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn value<T1>(self, value: T1) -> ClientAttributeBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            {
                let (v0,) = self.0;
                ClientAttributeBuilder((v0, value))
            }
        }

        impl<T0, T1> ClientAttributeBuilder<(T0, T1)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ClientAttribute].
            #[inline]
            pub fn finish(
                self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ClientAttribute>
            where
                Self: ::planus::WriteAsOffset<ClientAttribute>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            T1: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
        > ::planus::WriteAs<::planus::Offset<ClientAttribute>>
            for ClientAttributeBuilder<(T0, T1)>
        {
            type Prepared = ::planus::Offset<ClientAttribute>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ClientAttribute> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            T1: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
        > ::planus::WriteAsOptional<::planus::Offset<ClientAttribute>>
            for ClientAttributeBuilder<(T0, T1)>
        {
            type Prepared = ::planus::Offset<ClientAttribute>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ClientAttribute>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
            T0: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
            T1: ::planus::WriteAs<::planus::Offset<self::ByteArray>>,
        > ::planus::WriteAsOffset<ClientAttribute> for ClientAttributeBuilder<(T0, T1)>
        {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ClientAttribute> {
                let (v0, v1) = &self.0;
                ClientAttribute::create(builder, v0, v1)
            }
        }

        /// Reference to a deserialized [ClientAttribute].
        #[derive(Copy, Clone)]
        pub struct ClientAttributeRef<'a>(::planus::table_reader::Table<'a>);

        impl<'a> ClientAttributeRef<'a> {
            /// Getter for the [`key` field](ClientAttribute#structfield.key).
            #[inline]
            pub fn key(&self) -> ::planus::Result<self::ByteArrayRef<'a>> {
                self.0.access_required(0, "ClientAttribute", "key")
            }

            /// Getter for the [`value` field](ClientAttribute#structfield.value).
            #[inline]
            pub fn value(&self) -> ::planus::Result<self::ByteArrayRef<'a>> {
                self.0.access_required(1, "ClientAttribute", "value")
            }
        }

        impl<'a> ::core::fmt::Debug for ClientAttributeRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("ClientAttributeRef");
                f.field("key", &self.key());
                f.field("value", &self.value());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<ClientAttributeRef<'a>> for ClientAttribute {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: ClientAttributeRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    key: ::planus::alloc::boxed::Box::new(::core::convert::TryInto::try_into(
                        value.key()?,
                    )?),
                    value: ::planus::alloc::boxed::Box::new(::core::convert::TryInto::try_into(
                        value.value()?,
                    )?),
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for ClientAttributeRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for ClientAttributeRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location(
                        "[ClientAttributeRef]",
                        "get",
                        buffer.offset_from_start,
                    )
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<ClientAttribute>> for ClientAttribute {
            type Value = ::planus::Offset<ClientAttribute>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<ClientAttribute>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for ClientAttributeRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[ClientAttributeRef]", "read_as_root", 0)
                })
            }
        }

        /// The table `ConnectionRecord` in the namespace `MoorConnections`
        ///
        /// Generated from these locations:
        /// * Table `ConnectionRecord` in the file `connections.fbs:27`
        #[derive(
            Clone,
            Debug,
            PartialEq,
            PartialOrd,
            Eq,
            Ord,
            Hash,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        pub struct ConnectionRecord {
            /// The field `client_id_high` in the table `ConnectionRecord`
            pub client_id_high: u64,
            /// The field `client_id_low` in the table `ConnectionRecord`
            pub client_id_low: u64,
            /// The field `connected_secs` in the table `ConnectionRecord`
            pub connected_secs: u64,
            /// The field `connected_nanos` in the table `ConnectionRecord`
            pub connected_nanos: u32,
            /// The field `last_activity_secs` in the table `ConnectionRecord`
            pub last_activity_secs: u64,
            /// The field `last_activity_nanos` in the table `ConnectionRecord`
            pub last_activity_nanos: u32,
            /// The field `last_ping_secs` in the table `ConnectionRecord`
            pub last_ping_secs: u64,
            /// The field `last_ping_nanos` in the table `ConnectionRecord`
            pub last_ping_nanos: u32,
            /// The field `hostname` in the table `ConnectionRecord`
            pub hostname: ::planus::alloc::string::String,
            /// The field `local_port` in the table `ConnectionRecord`
            pub local_port: u16,
            /// The field `remote_port` in the table `ConnectionRecord`
            pub remote_port: u16,
            /// The field `acceptable_content_types` in the table `ConnectionRecord`
            pub acceptable_content_types: ::planus::alloc::vec::Vec<self::ByteArray>,
            /// The field `client_attributes` in the table `ConnectionRecord`
            pub client_attributes: ::planus::alloc::vec::Vec<self::ClientAttribute>,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for ConnectionRecord {
            fn default() -> Self {
                Self {
                    client_id_high: 0,
                    client_id_low: 0,
                    connected_secs: 0,
                    connected_nanos: 0,
                    last_activity_secs: 0,
                    last_activity_nanos: 0,
                    last_ping_secs: 0,
                    last_ping_nanos: 0,
                    hostname: ::core::default::Default::default(),
                    local_port: 0,
                    remote_port: 0,
                    acceptable_content_types: ::core::default::Default::default(),
                    client_attributes: ::core::default::Default::default(),
                }
            }
        }

        impl ConnectionRecord {
            /// Creates a [ConnectionRecordBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> ConnectionRecordBuilder<()> {
                ConnectionRecordBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_client_id_high: impl ::planus::WriteAsDefault<u64, u64>,
                field_client_id_low: impl ::planus::WriteAsDefault<u64, u64>,
                field_connected_secs: impl ::planus::WriteAsDefault<u64, u64>,
                field_connected_nanos: impl ::planus::WriteAsDefault<u32, u32>,
                field_last_activity_secs: impl ::planus::WriteAsDefault<u64, u64>,
                field_last_activity_nanos: impl ::planus::WriteAsDefault<u32, u32>,
                field_last_ping_secs: impl ::planus::WriteAsDefault<u64, u64>,
                field_last_ping_nanos: impl ::planus::WriteAsDefault<u32, u32>,
                field_hostname: impl ::planus::WriteAs<::planus::Offset<str>>,
                field_local_port: impl ::planus::WriteAsDefault<u16, u16>,
                field_remote_port: impl ::planus::WriteAsDefault<u16, u16>,
                field_acceptable_content_types: impl ::planus::WriteAs<
                    ::planus::Offset<[::planus::Offset<self::ByteArray>]>,
                >,
                field_client_attributes: impl ::planus::WriteAs<
                    ::planus::Offset<[::planus::Offset<self::ClientAttribute>]>,
                >,
            ) -> ::planus::Offset<Self> {
                let prepared_client_id_high = field_client_id_high.prepare(builder, &0);
                let prepared_client_id_low = field_client_id_low.prepare(builder, &0);
                let prepared_connected_secs = field_connected_secs.prepare(builder, &0);
                let prepared_connected_nanos = field_connected_nanos.prepare(builder, &0);
                let prepared_last_activity_secs = field_last_activity_secs.prepare(builder, &0);
                let prepared_last_activity_nanos = field_last_activity_nanos.prepare(builder, &0);
                let prepared_last_ping_secs = field_last_ping_secs.prepare(builder, &0);
                let prepared_last_ping_nanos = field_last_ping_nanos.prepare(builder, &0);
                let prepared_hostname = field_hostname.prepare(builder);
                let prepared_local_port = field_local_port.prepare(builder, &0);
                let prepared_remote_port = field_remote_port.prepare(builder, &0);
                let prepared_acceptable_content_types =
                    field_acceptable_content_types.prepare(builder);
                let prepared_client_attributes = field_client_attributes.prepare(builder);

                let mut table_writer: ::planus::table_writer::TableWriter<30> =
                    ::core::default::Default::default();
                if prepared_client_id_high.is_some() {
                    table_writer.write_entry::<u64>(0);
                }
                if prepared_client_id_low.is_some() {
                    table_writer.write_entry::<u64>(1);
                }
                if prepared_connected_secs.is_some() {
                    table_writer.write_entry::<u64>(2);
                }
                if prepared_last_activity_secs.is_some() {
                    table_writer.write_entry::<u64>(4);
                }
                if prepared_last_ping_secs.is_some() {
                    table_writer.write_entry::<u64>(6);
                }
                if prepared_connected_nanos.is_some() {
                    table_writer.write_entry::<u32>(3);
                }
                if prepared_last_activity_nanos.is_some() {
                    table_writer.write_entry::<u32>(5);
                }
                if prepared_last_ping_nanos.is_some() {
                    table_writer.write_entry::<u32>(7);
                }
                table_writer.write_entry::<::planus::Offset<str>>(8);
                table_writer
                    .write_entry::<::planus::Offset<[::planus::Offset<self::ByteArray>]>>(11);
                table_writer
                    .write_entry::<::planus::Offset<[::planus::Offset<self::ClientAttribute>]>>(12);
                if prepared_local_port.is_some() {
                    table_writer.write_entry::<u16>(9);
                }
                if prepared_remote_port.is_some() {
                    table_writer.write_entry::<u16>(10);
                }

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        if let ::core::option::Option::Some(prepared_client_id_high) =
                            prepared_client_id_high
                        {
                            object_writer.write::<_, _, 8>(&prepared_client_id_high);
                        }
                        if let ::core::option::Option::Some(prepared_client_id_low) =
                            prepared_client_id_low
                        {
                            object_writer.write::<_, _, 8>(&prepared_client_id_low);
                        }
                        if let ::core::option::Option::Some(prepared_connected_secs) =
                            prepared_connected_secs
                        {
                            object_writer.write::<_, _, 8>(&prepared_connected_secs);
                        }
                        if let ::core::option::Option::Some(prepared_last_activity_secs) =
                            prepared_last_activity_secs
                        {
                            object_writer.write::<_, _, 8>(&prepared_last_activity_secs);
                        }
                        if let ::core::option::Option::Some(prepared_last_ping_secs) =
                            prepared_last_ping_secs
                        {
                            object_writer.write::<_, _, 8>(&prepared_last_ping_secs);
                        }
                        if let ::core::option::Option::Some(prepared_connected_nanos) =
                            prepared_connected_nanos
                        {
                            object_writer.write::<_, _, 4>(&prepared_connected_nanos);
                        }
                        if let ::core::option::Option::Some(prepared_last_activity_nanos) =
                            prepared_last_activity_nanos
                        {
                            object_writer.write::<_, _, 4>(&prepared_last_activity_nanos);
                        }
                        if let ::core::option::Option::Some(prepared_last_ping_nanos) =
                            prepared_last_ping_nanos
                        {
                            object_writer.write::<_, _, 4>(&prepared_last_ping_nanos);
                        }
                        object_writer.write::<_, _, 4>(&prepared_hostname);
                        object_writer.write::<_, _, 4>(&prepared_acceptable_content_types);
                        object_writer.write::<_, _, 4>(&prepared_client_attributes);
                        if let ::core::option::Option::Some(prepared_local_port) =
                            prepared_local_port
                        {
                            object_writer.write::<_, _, 2>(&prepared_local_port);
                        }
                        if let ::core::option::Option::Some(prepared_remote_port) =
                            prepared_remote_port
                        {
                            object_writer.write::<_, _, 2>(&prepared_remote_port);
                        }
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<ConnectionRecord>> for ConnectionRecord {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionRecord> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<ConnectionRecord>> for ConnectionRecord {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ConnectionRecord>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<ConnectionRecord> for ConnectionRecord {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionRecord> {
                ConnectionRecord::create(
                    builder,
                    self.client_id_high,
                    self.client_id_low,
                    self.connected_secs,
                    self.connected_nanos,
                    self.last_activity_secs,
                    self.last_activity_nanos,
                    self.last_ping_secs,
                    self.last_ping_nanos,
                    &self.hostname,
                    self.local_port,
                    self.remote_port,
                    &self.acceptable_content_types,
                    &self.client_attributes,
                )
            }
        }

        /// Builder for serializing an instance of the [ConnectionRecord] type.
        ///
        /// Can be created using the [ConnectionRecord::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct ConnectionRecordBuilder<State>(State);

        impl ConnectionRecordBuilder<()> {
            /// Setter for the [`client_id_high` field](ConnectionRecord#structfield.client_id_high).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn client_id_high<T0>(self, value: T0) -> ConnectionRecordBuilder<(T0,)>
            where
                T0: ::planus::WriteAsDefault<u64, u64>,
            {
                ConnectionRecordBuilder((value,))
            }

            /// Sets the [`client_id_high` field](ConnectionRecord#structfield.client_id_high) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn client_id_high_as_default(
                self,
            ) -> ConnectionRecordBuilder<(::planus::DefaultValue,)> {
                self.client_id_high(::planus::DefaultValue)
            }
        }

        impl<T0> ConnectionRecordBuilder<(T0,)> {
            /// Setter for the [`client_id_low` field](ConnectionRecord#structfield.client_id_low).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn client_id_low<T1>(self, value: T1) -> ConnectionRecordBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0,) = self.0;
                ConnectionRecordBuilder((v0, value))
            }

            /// Sets the [`client_id_low` field](ConnectionRecord#structfield.client_id_low) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn client_id_low_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, ::planus::DefaultValue)> {
                self.client_id_low(::planus::DefaultValue)
            }
        }

        impl<T0, T1> ConnectionRecordBuilder<(T0, T1)> {
            /// Setter for the [`connected_secs` field](ConnectionRecord#structfield.connected_secs).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn connected_secs<T2>(self, value: T2) -> ConnectionRecordBuilder<(T0, T1, T2)>
            where
                T2: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1) = self.0;
                ConnectionRecordBuilder((v0, v1, value))
            }

            /// Sets the [`connected_secs` field](ConnectionRecord#structfield.connected_secs) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn connected_secs_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, ::planus::DefaultValue)> {
                self.connected_secs(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2> ConnectionRecordBuilder<(T0, T1, T2)> {
            /// Setter for the [`connected_nanos` field](ConnectionRecord#structfield.connected_nanos).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn connected_nanos<T3>(self, value: T3) -> ConnectionRecordBuilder<(T0, T1, T2, T3)>
            where
                T3: ::planus::WriteAsDefault<u32, u32>,
            {
                let (v0, v1, v2) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, value))
            }

            /// Sets the [`connected_nanos` field](ConnectionRecord#structfield.connected_nanos) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn connected_nanos_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                self.connected_nanos(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3> ConnectionRecordBuilder<(T0, T1, T2, T3)> {
            /// Setter for the [`last_activity_secs` field](ConnectionRecord#structfield.last_activity_secs).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_activity_secs<T4>(
                self,
                value: T4,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4)>
            where
                T4: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, value))
            }

            /// Sets the [`last_activity_secs` field](ConnectionRecord#structfield.last_activity_secs) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_activity_secs_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                self.last_activity_secs(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4> ConnectionRecordBuilder<(T0, T1, T2, T3, T4)> {
            /// Setter for the [`last_activity_nanos` field](ConnectionRecord#structfield.last_activity_nanos).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_activity_nanos<T5>(
                self,
                value: T5,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5)>
            where
                T5: ::planus::WriteAsDefault<u32, u32>,
            {
                let (v0, v1, v2, v3, v4) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, value))
            }

            /// Sets the [`last_activity_nanos` field](ConnectionRecord#structfield.last_activity_nanos) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_activity_nanos_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)> {
                self.last_activity_nanos(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5)> {
            /// Setter for the [`last_ping_secs` field](ConnectionRecord#structfield.last_ping_secs).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_ping_secs<T6>(
                self,
                value: T6,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6)>
            where
                T6: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3, v4, v5) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, value))
            }

            /// Sets the [`last_ping_secs` field](ConnectionRecord#structfield.last_ping_secs) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_ping_secs_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)>
            {
                self.last_ping_secs(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
            /// Setter for the [`last_ping_nanos` field](ConnectionRecord#structfield.last_ping_nanos).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_ping_nanos<T7>(
                self,
                value: T7,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)>
            where
                T7: ::planus::WriteAsDefault<u32, u32>,
            {
                let (v0, v1, v2, v3, v4, v5, v6) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, value))
            }

            /// Sets the [`last_ping_nanos` field](ConnectionRecord#structfield.last_ping_nanos) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn last_ping_nanos_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, ::planus::DefaultValue)>
            {
                self.last_ping_nanos(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)> {
            /// Setter for the [`hostname` field](ConnectionRecord#structfield.hostname).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn hostname<T8>(
                self,
                value: T8,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
            where
                T8: ::planus::WriteAs<::planus::Offset<str>>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, v7, value))
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8>
            ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
        {
            /// Setter for the [`local_port` field](ConnectionRecord#structfield.local_port).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn local_port<T9>(
                self,
                value: T9,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
            where
                T9: ::planus::WriteAsDefault<u16, u16>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, value))
            }

            /// Sets the [`local_port` field](ConnectionRecord#structfield.local_port) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn local_port_as_default(
                self,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, ::planus::DefaultValue)>
            {
                self.local_port(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9>
            ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
        {
            /// Setter for the [`remote_port` field](ConnectionRecord#structfield.remote_port).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn remote_port<T10>(
                self,
                value: T10,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
            where
                T10: ::planus::WriteAsDefault<u16, u16>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, value))
            }

            /// Sets the [`remote_port` field](ConnectionRecord#structfield.remote_port) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn remote_port_as_default(
                self,
            ) -> ConnectionRecordBuilder<(
                T0,
                T1,
                T2,
                T3,
                T4,
                T5,
                T6,
                T7,
                T8,
                T9,
                ::planus::DefaultValue,
            )> {
                self.remote_port(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
            ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
        {
            /// Setter for the [`acceptable_content_types` field](ConnectionRecord#structfield.acceptable_content_types).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn acceptable_content_types<T11>(
                self,
                value: T11,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
            where
                T11: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ByteArray>]>>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, value))
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
            ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
        {
            /// Setter for the [`client_attributes` field](ConnectionRecord#structfield.client_attributes).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn client_attributes<T12>(
                self,
                value: T12,
            ) -> ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
            where
                T12: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ClientAttribute>]>>,
            {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11) = self.0;
                ConnectionRecordBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, value))
            }
        }

        impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
            ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
        {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ConnectionRecord].
            #[inline]
            pub fn finish(
                self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionRecord>
            where
                Self: ::planus::WriteAsOffset<ConnectionRecord>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u64, u64>,
            T1: ::planus::WriteAsDefault<u64, u64>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u32, u32>,
            T4: ::planus::WriteAsDefault<u64, u64>,
            T5: ::planus::WriteAsDefault<u32, u32>,
            T6: ::planus::WriteAsDefault<u64, u64>,
            T7: ::planus::WriteAsDefault<u32, u32>,
            T8: ::planus::WriteAs<::planus::Offset<str>>,
            T9: ::planus::WriteAsDefault<u16, u16>,
            T10: ::planus::WriteAsDefault<u16, u16>,
            T11: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ByteArray>]>>,
            T12: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ClientAttribute>]>>,
        > ::planus::WriteAs<::planus::Offset<ConnectionRecord>>
            for ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
        {
            type Prepared = ::planus::Offset<ConnectionRecord>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionRecord> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u64, u64>,
            T1: ::planus::WriteAsDefault<u64, u64>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u32, u32>,
            T4: ::planus::WriteAsDefault<u64, u64>,
            T5: ::planus::WriteAsDefault<u32, u32>,
            T6: ::planus::WriteAsDefault<u64, u64>,
            T7: ::planus::WriteAsDefault<u32, u32>,
            T8: ::planus::WriteAs<::planus::Offset<str>>,
            T9: ::planus::WriteAsDefault<u16, u16>,
            T10: ::planus::WriteAsDefault<u16, u16>,
            T11: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ByteArray>]>>,
            T12: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ClientAttribute>]>>,
        > ::planus::WriteAsOptional<::planus::Offset<ConnectionRecord>>
            for ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
        {
            type Prepared = ::planus::Offset<ConnectionRecord>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ConnectionRecord>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u64, u64>,
            T1: ::planus::WriteAsDefault<u64, u64>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u32, u32>,
            T4: ::planus::WriteAsDefault<u64, u64>,
            T5: ::planus::WriteAsDefault<u32, u32>,
            T6: ::planus::WriteAsDefault<u64, u64>,
            T7: ::planus::WriteAsDefault<u32, u32>,
            T8: ::planus::WriteAs<::planus::Offset<str>>,
            T9: ::planus::WriteAsDefault<u16, u16>,
            T10: ::planus::WriteAsDefault<u16, u16>,
            T11: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ByteArray>]>>,
            T12: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ClientAttribute>]>>,
        > ::planus::WriteAsOffset<ConnectionRecord>
            for ConnectionRecordBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
        {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionRecord> {
                let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12) = &self.0;
                ConnectionRecord::create(
                    builder, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12,
                )
            }
        }

        /// Reference to a deserialized [ConnectionRecord].
        #[derive(Copy, Clone)]
        pub struct ConnectionRecordRef<'a>(::planus::table_reader::Table<'a>);

        impl<'a> ConnectionRecordRef<'a> {
            /// Getter for the [`client_id_high` field](ConnectionRecord#structfield.client_id_high).
            #[inline]
            pub fn client_id_high(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(
                    self.0
                        .access(0, "ConnectionRecord", "client_id_high")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`client_id_low` field](ConnectionRecord#structfield.client_id_low).
            #[inline]
            pub fn client_id_low(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(
                    self.0
                        .access(1, "ConnectionRecord", "client_id_low")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`connected_secs` field](ConnectionRecord#structfield.connected_secs).
            #[inline]
            pub fn connected_secs(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(
                    self.0
                        .access(2, "ConnectionRecord", "connected_secs")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`connected_nanos` field](ConnectionRecord#structfield.connected_nanos).
            #[inline]
            pub fn connected_nanos(&self) -> ::planus::Result<u32> {
                ::core::result::Result::Ok(
                    self.0
                        .access(3, "ConnectionRecord", "connected_nanos")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`last_activity_secs` field](ConnectionRecord#structfield.last_activity_secs).
            #[inline]
            pub fn last_activity_secs(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(
                    self.0
                        .access(4, "ConnectionRecord", "last_activity_secs")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`last_activity_nanos` field](ConnectionRecord#structfield.last_activity_nanos).
            #[inline]
            pub fn last_activity_nanos(&self) -> ::planus::Result<u32> {
                ::core::result::Result::Ok(
                    self.0
                        .access(5, "ConnectionRecord", "last_activity_nanos")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`last_ping_secs` field](ConnectionRecord#structfield.last_ping_secs).
            #[inline]
            pub fn last_ping_secs(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(
                    self.0
                        .access(6, "ConnectionRecord", "last_ping_secs")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`last_ping_nanos` field](ConnectionRecord#structfield.last_ping_nanos).
            #[inline]
            pub fn last_ping_nanos(&self) -> ::planus::Result<u32> {
                ::core::result::Result::Ok(
                    self.0
                        .access(7, "ConnectionRecord", "last_ping_nanos")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`hostname` field](ConnectionRecord#structfield.hostname).
            #[inline]
            pub fn hostname(&self) -> ::planus::Result<&'a ::core::primitive::str> {
                self.0.access_required(8, "ConnectionRecord", "hostname")
            }

            /// Getter for the [`local_port` field](ConnectionRecord#structfield.local_port).
            #[inline]
            pub fn local_port(&self) -> ::planus::Result<u16> {
                ::core::result::Result::Ok(
                    self.0
                        .access(9, "ConnectionRecord", "local_port")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`remote_port` field](ConnectionRecord#structfield.remote_port).
            #[inline]
            pub fn remote_port(&self) -> ::planus::Result<u16> {
                ::core::result::Result::Ok(
                    self.0
                        .access(10, "ConnectionRecord", "remote_port")?
                        .unwrap_or(0),
                )
            }

            /// Getter for the [`acceptable_content_types` field](ConnectionRecord#structfield.acceptable_content_types).
            #[inline]
            pub fn acceptable_content_types(
                &self,
            ) -> ::planus::Result<::planus::Vector<'a, ::planus::Result<self::ByteArrayRef<'a>>>>
            {
                self.0
                    .access_required(11, "ConnectionRecord", "acceptable_content_types")
            }

            /// Getter for the [`client_attributes` field](ConnectionRecord#structfield.client_attributes).
            #[inline]
            pub fn client_attributes(
                &self,
            ) -> ::planus::Result<
                ::planus::Vector<'a, ::planus::Result<self::ClientAttributeRef<'a>>>,
            > {
                self.0
                    .access_required(12, "ConnectionRecord", "client_attributes")
            }
        }

        impl<'a> ::core::fmt::Debug for ConnectionRecordRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("ConnectionRecordRef");
                f.field("client_id_high", &self.client_id_high());
                f.field("client_id_low", &self.client_id_low());
                f.field("connected_secs", &self.connected_secs());
                f.field("connected_nanos", &self.connected_nanos());
                f.field("last_activity_secs", &self.last_activity_secs());
                f.field("last_activity_nanos", &self.last_activity_nanos());
                f.field("last_ping_secs", &self.last_ping_secs());
                f.field("last_ping_nanos", &self.last_ping_nanos());
                f.field("hostname", &self.hostname());
                f.field("local_port", &self.local_port());
                f.field("remote_port", &self.remote_port());
                f.field("acceptable_content_types", &self.acceptable_content_types());
                f.field("client_attributes", &self.client_attributes());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<ConnectionRecordRef<'a>> for ConnectionRecord {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: ConnectionRecordRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    client_id_high: ::core::convert::TryInto::try_into(value.client_id_high()?)?,
                    client_id_low: ::core::convert::TryInto::try_into(value.client_id_low()?)?,
                    connected_secs: ::core::convert::TryInto::try_into(value.connected_secs()?)?,
                    connected_nanos: ::core::convert::TryInto::try_into(value.connected_nanos()?)?,
                    last_activity_secs: ::core::convert::TryInto::try_into(
                        value.last_activity_secs()?,
                    )?,
                    last_activity_nanos: ::core::convert::TryInto::try_into(
                        value.last_activity_nanos()?,
                    )?,
                    last_ping_secs: ::core::convert::TryInto::try_into(value.last_ping_secs()?)?,
                    last_ping_nanos: ::core::convert::TryInto::try_into(value.last_ping_nanos()?)?,
                    hostname: ::core::convert::Into::into(value.hostname()?),
                    local_port: ::core::convert::TryInto::try_into(value.local_port()?)?,
                    remote_port: ::core::convert::TryInto::try_into(value.remote_port()?)?,
                    acceptable_content_types: value.acceptable_content_types()?.to_vec_result()?,
                    client_attributes: value.client_attributes()?.to_vec_result()?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for ConnectionRecordRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for ConnectionRecordRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location(
                        "[ConnectionRecordRef]",
                        "get",
                        buffer.offset_from_start,
                    )
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<ConnectionRecord>> for ConnectionRecord {
            type Value = ::planus::Offset<ConnectionRecord>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<ConnectionRecord>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for ConnectionRecordRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[ConnectionRecordRef]", "read_as_root", 0)
                })
            }
        }

        /// The table `ConnectionsRecords` in the namespace `MoorConnections`
        ///
        /// Generated from these locations:
        /// * Table `ConnectionsRecords` in the file `connections.fbs:46`
        #[derive(
            Clone,
            Debug,
            PartialEq,
            PartialOrd,
            Eq,
            Ord,
            Hash,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        pub struct ConnectionsRecords {
            /// The field `connections` in the table `ConnectionsRecords`
            pub connections: ::planus::alloc::vec::Vec<self::ConnectionRecord>,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for ConnectionsRecords {
            fn default() -> Self {
                Self {
                    connections: ::core::default::Default::default(),
                }
            }
        }

        impl ConnectionsRecords {
            /// Creates a [ConnectionsRecordsBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> ConnectionsRecordsBuilder<()> {
                ConnectionsRecordsBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_connections: impl ::planus::WriteAs<
                    ::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>,
                >,
            ) -> ::planus::Offset<Self> {
                let prepared_connections = field_connections.prepare(builder);

                let mut table_writer: ::planus::table_writer::TableWriter<6> =
                    ::core::default::Default::default();
                table_writer
                    .write_entry::<::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>>(0);

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        object_writer.write::<_, _, 4>(&prepared_connections);
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<ConnectionsRecords>> for ConnectionsRecords {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionsRecords> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<ConnectionsRecords>> for ConnectionsRecords {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ConnectionsRecords>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<ConnectionsRecords> for ConnectionsRecords {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionsRecords> {
                ConnectionsRecords::create(builder, &self.connections)
            }
        }

        /// Builder for serializing an instance of the [ConnectionsRecords] type.
        ///
        /// Can be created using the [ConnectionsRecords::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct ConnectionsRecordsBuilder<State>(State);

        impl ConnectionsRecordsBuilder<()> {
            /// Setter for the [`connections` field](ConnectionsRecords#structfield.connections).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn connections<T0>(self, value: T0) -> ConnectionsRecordsBuilder<(T0,)>
            where
                T0: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>>,
            {
                ConnectionsRecordsBuilder((value,))
            }
        }

        impl<T0> ConnectionsRecordsBuilder<(T0,)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ConnectionsRecords].
            #[inline]
            pub fn finish(
                self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionsRecords>
            where
                Self: ::planus::WriteAsOffset<ConnectionsRecords>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>>>
            ::planus::WriteAs<::planus::Offset<ConnectionsRecords>>
            for ConnectionsRecordsBuilder<(T0,)>
        {
            type Prepared = ::planus::Offset<ConnectionsRecords>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionsRecords> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>>>
            ::planus::WriteAsOptional<::planus::Offset<ConnectionsRecords>>
            for ConnectionsRecordsBuilder<(T0,)>
        {
            type Prepared = ::planus::Offset<ConnectionsRecords>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<ConnectionsRecords>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<T0: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::ConnectionRecord>]>>>
            ::planus::WriteAsOffset<ConnectionsRecords> for ConnectionsRecordsBuilder<(T0,)>
        {
            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::planus::Offset<ConnectionsRecords> {
                let (v0,) = &self.0;
                ConnectionsRecords::create(builder, v0)
            }
        }

        /// Reference to a deserialized [ConnectionsRecords].
        #[derive(Copy, Clone)]
        pub struct ConnectionsRecordsRef<'a>(::planus::table_reader::Table<'a>);

        impl<'a> ConnectionsRecordsRef<'a> {
            /// Getter for the [`connections` field](ConnectionsRecords#structfield.connections).
            #[inline]
            pub fn connections(
                &self,
            ) -> ::planus::Result<
                ::planus::Vector<'a, ::planus::Result<self::ConnectionRecordRef<'a>>>,
            > {
                self.0
                    .access_required(0, "ConnectionsRecords", "connections")
            }
        }

        impl<'a> ::core::fmt::Debug for ConnectionsRecordsRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("ConnectionsRecordsRef");
                f.field("connections", &self.connections());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<ConnectionsRecordsRef<'a>> for ConnectionsRecords {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: ConnectionsRecordsRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    connections: value.connections()?.to_vec_result()?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for ConnectionsRecordsRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for ConnectionsRecordsRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location(
                        "[ConnectionsRecordsRef]",
                        "get",
                        buffer.offset_from_start,
                    )
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<ConnectionsRecords>> for ConnectionsRecords {
            type Value = ::planus::Offset<ConnectionsRecords>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<ConnectionsRecords>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for ConnectionsRecordsRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[ConnectionsRecordsRef]", "read_as_root", 0)
                })
            }
        }
    }
}
