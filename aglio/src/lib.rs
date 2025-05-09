use std::fmt::Display;
use std::marker::PhantomData;
use std::str::Utf8Error;
use serde::de::{DeserializeOwned, DeserializeSeed, Visitor};
use serde::{Deserialize, Serialize, Serializer};


#[derive(Copy, Clone)]
pub enum Endianess {
    Little,
    Big,
}
pub struct AglioConfig<'a, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone = crc::Table<1> > {
    pub endianess: Endianess,
    pub packet_start: &'a [u8],
    pub body_crc: Option<crc::Crc<W, I>>,
    pub phantom_data: PhantomData<S>,
}
impl<'a, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, > AglioConfig<'a, S, u16> {
    const DEFAULT: Self = Self {
        endianess: Endianess::Little,
        packet_start: &[0xAA, 0x55],
        body_crc: Some(crc::Crc::<u16>::new(&crc::CRC_16_USB)),
        phantom_data: PhantomData,
    };
}
impl<'a, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, > Default for AglioConfig<'a, S, u16>{
    fn default() -> Self {
        AglioConfig::DEFAULT
    }
}
impl<'a, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone> Clone for AglioConfig<'a, S, W, I> {
    fn clone(&self) -> Self {
        Self {
            endianess: self.endianess,
            packet_start: self.packet_start,
            body_crc: self.body_crc.clone(),
            phantom_data: PhantomData,
        }
    }
}


#[derive(thiserror::Error, Debug)]
pub enum SerializeError {
    #[error("Cannot automatically infer data type")]
    NotDescriptive,
    #[error("Array, String or Sequence is too long")]
    TooLong,
    #[error("{0}")]
    Custom(String),
}
impl serde::ser::Error for SerializeError{
    fn custom<T>(msg: T) -> Self
    where
        T: Display
    {
        Self::Custom(msg.to_string())
    }
}


#[inline]
pub fn serialize<S: serde::Serialize>(value: &S) -> Result<Vec<u8>, SerializeError> {
    serialize_with_config(AglioConfig::<u32, u16>::DEFAULT, value)
}

pub fn serialize_with_config<'a, S: serde::Serialize, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, >(config: AglioConfig<'a, Size, u16, crc::Table<1>>, value: &S) -> Result<Vec<u8>, SerializeError> {
    struct AglioSerializer<'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, > {
        config: AglioConfig<'a, S, W, I>,
        data: Vec<u8>,
    }

    impl<'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> AglioSerializer<'a, W, I, S> {
        fn serialize_usize_as_u32(&mut self, len: usize) -> Result<(), SerializeError> {
            match S::try_from(len) {
                Ok(len) => {
                    len.serialize(&mut*self)
                },
                Err(_) => {
                    Err(serde::ser::Error::custom("Sequence length too long"))
                }
            }
        }
    }
    struct SerializeSeq<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> {
        elements: usize,
        serializer: &'de mut AglioSerializer<'a, W, I, S>,
        intermediate_serializer: AglioSerializer<'a, W, I, S>,
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeSeq for SerializeSeq<'de, 'a, W, I, S> {
        type Ok = ();
        type Error = SerializeError;

        fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            self.elements += 1;
            value.serialize(&mut self.intermediate_serializer)
        }

        fn end(mut self) -> Result<Self::Ok, Self::Error> {
            self.serializer.serialize_usize_as_u32(self.elements)?;
            self.serializer.data.append(&mut self.intermediate_serializer.data);
            Ok(())
        }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeTuple for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = <Self as serde::ser::Serializer>::Ok;
        type Error = <Self as serde::ser::Serializer>::Error;

        #[inline]
        fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut**self)
        }
        #[inline]
        fn end(mut self) -> Result<Self::Ok, Self::Error> { Ok(()) }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeTupleStruct for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = <Self as serde::ser::Serializer>::Ok;
        type Error = <Self as serde::ser::Serializer>::Error;

        #[inline]
        fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut**self)
        }
        #[inline]
        fn end(mut self) -> Result<Self::Ok, Self::Error> { Ok(()) }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeTupleVariant for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = <Self as serde::ser::Serializer>::Ok;
        type Error = <Self as serde::ser::Serializer>::Error;

        #[inline]
        fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut**self)
        }
        #[inline]
        fn end(mut self) -> Result<Self::Ok, Self::Error> { Ok(()) }
    }
    struct SerializeMap<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> {
        elements: usize,
        serializer: &'de mut AglioSerializer<'a, W, I, S>,
        intermediate_serializer: AglioSerializer<'a, W, I, S>,
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeMap for SerializeMap<'de, 'a, W, I, S> {
        type Ok = ();
        type Error = SerializeError;

        fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            self.elements += 1;
            key.serialize(&mut self.intermediate_serializer)
        }

        fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut self.intermediate_serializer)
        }

        fn end(mut self) -> Result<Self::Ok, Self::Error> {
            self.serializer.serialize_usize_as_u32(self.elements)?;
            self.serializer.data.append(&mut self.intermediate_serializer.data);
            Ok(())
        }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeStruct for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = <Self as serde::ser::Serializer>::Ok;
        type Error = <Self as serde::ser::Serializer>::Error;

        #[inline]
        fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut**self)
        }
        #[inline]
        fn end(mut self) -> Result<Self::Ok, Self::Error> { Ok(()) }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::ser::SerializeStructVariant for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = <Self as serde::ser::Serializer>::Ok;
        type Error = <Self as serde::ser::Serializer>::Error;

        #[inline]
        fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
        where
            T: ?Sized + Serialize
        {
            value.serialize(&mut**self)
        }
        #[inline]
        fn end(mut self) -> Result<Self::Ok, Self::Error> { Ok(()) }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, S: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::Serializer for &'de mut AglioSerializer<'a, W, I, S> {
        type Ok = ();
        type Error = SerializeError;
        type SerializeSeq = SerializeSeq<'de, 'a, W, I, S>;
        type SerializeTuple = &'de mut AglioSerializer<'a, W, I, S>;
        type SerializeTupleStruct = &'de mut AglioSerializer<'a, W, I, S>;
        type SerializeTupleVariant = &'de mut AglioSerializer<'a, W, I, S>;
        type SerializeMap = SerializeMap<'de, 'a, W, I, S>;
        type SerializeStruct = &'de mut AglioSerializer<'a, W, I, S>;
        type SerializeStructVariant = &'de mut AglioSerializer<'a, W, I, S>;

        fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
            self.data.push(u8::from(v));
            Ok(())
        }
        fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }
        fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
            match self.config.endianess {
                Endianess::Little => self.data.extend_from_slice(&v.to_le_bytes()),
                Endianess::Big => self.data.extend_from_slice(&v.to_be_bytes()),
            }
            Ok(())
        }

        #[inline]
        fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
            self.serialize_str(v.to_string().as_str())?;
            Ok(())
        }

        #[inline]
        fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
            self.serialize_bytes(v.as_bytes())
        }

        fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
            self.serialize_usize_as_u32(v.len())?;
            self.data.extend_from_slice(v);
            Ok(())
        }

        #[inline]
        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            self.data.push(0);
            Ok(())
        }

        fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize
        {
            self.data.push(1);
            value.serialize(self)?;
            Ok(())
        }

        #[inline]
        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> { Ok(())}

        #[inline]
        fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> { Ok(()) }

        #[inline]
        fn serialize_unit_variant(self, name: &'static str, variant_index: u32, variant: &'static str) -> Result<Self::Ok, Self::Error> { self.serialize_u32(variant_index) }

        #[inline]
        fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize
        { value.serialize(self) }

        fn serialize_newtype_variant<T>(self, name: &'static str, variant_index: u32, variant: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: ?Sized + Serialize
        {
            self.serialize_u32(variant_index)?;
            value.serialize(self)
        }

        fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Ok(SerializeSeq{
                elements: 0,
                intermediate_serializer: AglioSerializer{
                    config: self.config.clone(),
                    data: Vec::new(),
                },
                serializer: self,
            })
        }

        #[inline]
        fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Ok(self)
        }

        #[inline]
        fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Ok(self)
        }

        #[inline]
        fn serialize_tuple_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Ok(self)
        }

        fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Ok(SerializeMap{
                elements: 0,
                intermediate_serializer: AglioSerializer{
                    config: self.config.clone(),
                    data: Vec::new(),
                },
                serializer: self,
            })
        }

        #[inline]
        fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
            Ok(self)
        }

        #[inline]
        fn serialize_struct_variant(self, name: &'static str, variant_index: u32, variant: &'static str, len: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
            Ok(self)
        }

        fn is_human_readable(&self) -> bool {
            false
        }
    }
    let mut serializer = AglioSerializer{
        config: config.clone(),
        data: Vec::new()
    };
    value.serialize(&mut serializer)?;
    let mut final_data = Vec::with_capacity(serializer.data.len() + config.packet_start.len() + if config.body_crc.is_some() { core::mem::size_of::<u16>() } else { 0 });
    final_data.extend_from_slice(config.packet_start);
    match u32::try_from(serializer.data.len()) {
        Ok(len) => {
            match config.endianess {
                Endianess::Little => final_data.extend_from_slice(&len.to_le_bytes()),
                Endianess::Big => final_data.extend_from_slice(&len.to_be_bytes()),
            }
        }
        Err(_) => {
            return Err(SerializeError::TooLong);
        }
    }
    let crc = config.body_crc.map(|v|v.checksum(serializer.data.as_slice()));
    final_data.append(&mut serializer.data);
    if let Some(crc) = crc {
        match config.endianess {
            Endianess::Little => final_data.extend_from_slice(&crc.to_le_bytes()),
            Endianess::Big => final_data.extend_from_slice(&crc.to_be_bytes()),
        }
    };
    Ok(final_data)
}

#[derive(thiserror::Error, Debug)]
pub enum DeserializeError {
    #[error("Cannot automatically infer data type")]
    NotDescriptive,
    #[error("Invalid data")]
    InvalidData,
    #[error("Incomplete or Bad data")]
    BadData,
    #[error("The checksum of the data does not match")]
    ChecksumError,
    #[error("Invalid size")]
    InvalidSize,
    #[error("Invalid Packet Start data")]
    InvalidPacketStart,
    #[error("Invalid String data: {0}")]
    InvalidUtf8(Utf8Error),
    #[error("Not enough data")]
    InvalidLength,
    #[error("{0}")]
    Custom(String),
}
impl serde::de::Error for DeserializeError{
    fn custom<T>(msg: T) -> Self
    where
        T: Display
    {
        DeserializeError::Custom(msg.to_string())
    }
}

#[inline]
pub fn deserialize<'de, S: serde::Deserialize<'de>>(data: &'de[u8]) -> Result<S, DeserializeError> {
    deserialize_with_config(AglioConfig::<u32, u16>::DEFAULT, data)
}
pub fn deserialize_with_config<'de, 'a, S: serde::Deserialize<'de>, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>>(config: AglioConfig<'a, Size, u16, crc::Table<1>>, data: &'de[u8]) -> Result<S, DeserializeError> {
    struct AglioDeserializer<'de, 'a, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone> {
        config: AglioConfig<'a, Size, W, I>,
        data: &'de[u8],
    }
    
    macro_rules! deserialize_num {
        ($ty:ty, $fn_name:ident, $visit_name:ident) => {
            fn $fn_name<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>
            {
                let (first, rest) = match self.data.split_first_chunk() {
                    Some(v) => v,
                    None => return Err(DeserializeError::InvalidLength),
                };
                self.data = rest;
                let value = match self.config.endianess {
                    Endianess::Little => {
                        <$ty>::from_le_bytes(*first)
                    },
                    Endianess::Big => {
                        <$ty>::from_be_bytes(*first)
                    }
                };
                visitor.$visit_name(value)
            }
        };
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> AglioDeserializer<'de, 'a, Size, W, I> {
        fn get_usize(&mut self) -> Result<usize, DeserializeError> {
            match Size::deserialize(&mut*self)?.try_into() {
                Ok(v) => Ok(v),
                Err(_) => Err(DeserializeError::InvalidSize), 
            }
        }
    }
    impl<'de, 'a, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::Deserializer<'de> for &mut AglioDeserializer<'de, 'a, Size, W, I> {
        type Error = DeserializeError;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            Err(DeserializeError::NotDescriptive)
        }

        fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            match self.data.split_first() {
                None => Err(crate::DeserializeError::InvalidLength),
                Some((0, rest)) => {
                    self.data = rest;
                    visitor.visit_bool(false)
                },
                Some((1, rest)) => {
                    self.data = rest;
                    visitor.visit_bool(true)
                }
                Some((_, rest)) => {
                    self.data = rest;
                    Err(DeserializeError::InvalidData)
                }
            }
        }
        deserialize_num!(i8, deserialize_i8, visit_i8);
        deserialize_num!(i16, deserialize_i16, visit_i16);
        deserialize_num!(i32, deserialize_i32, visit_i32);
        deserialize_num!(i64, deserialize_i64, visit_i64);
        deserialize_num!(i128, deserialize_i128, visit_i128);
        deserialize_num!(u8, deserialize_u8, visit_u8);
        deserialize_num!(u16, deserialize_u16, visit_u16);
        deserialize_num!(u32, deserialize_u32, visit_u32);
        deserialize_num!(u64, deserialize_u64, visit_u64);
        deserialize_num!(u128, deserialize_u128, visit_u128);
        deserialize_num!(f32, deserialize_f32, visit_f32);
        deserialize_num!(f64, deserialize_f64, visit_f64);


        fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            let data = match self.data.split_at_checked(size) {
                Some((first, rest)) => {
                    self.data = rest;
                    first
                }
                None => return Err(DeserializeError::InvalidLength),
            };
            let string = std::str::from_utf8(data).map_err(DeserializeError::InvalidUtf8)?;
            match string.chars().next() {
                None => return Err(DeserializeError::InvalidLength),
                Some(first) => visitor.visit_char(first)
            }
        }

        fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            let data = match self.data.split_at_checked(usize::from(size)) {
                Some((first, rest)) => {
                    self.data = rest;
                    first
                }
                None => return Err(DeserializeError::InvalidLength),
            };
            let string = std::str::from_utf8(data).map_err(DeserializeError::InvalidUtf8)?;
            visitor.visit_borrowed_str(string)
        }

        fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            let data = match self.data.split_at_checked(usize::from(size)) {
                Some((first, rest)) => {
                    self.data = rest;
                    first
                }
                None => return Err(DeserializeError::InvalidLength),
            };
            let string = std::str::from_utf8(data).map_err(DeserializeError::InvalidUtf8)?;
            visitor.visit_string(string.to_string())
        }

        fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            let data = match self.data.split_at_checked(usize::from(size)) {
                Some((first, rest)) => {
                    self.data = rest;
                    first
                }
                None => return Err(DeserializeError::InvalidLength),
            };
            visitor.visit_borrowed_bytes(data)
        }

        fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            let data = match self.data.split_at_checked(usize::from(size)) {
                Some((first, rest)) => {
                    self.data = rest;
                    first
                }
                None => return Err(DeserializeError::InvalidLength),
            };
            visitor.visit_byte_buf(Vec::from(data))
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            match self.data.split_first() {
                None => Err(DeserializeError::InvalidLength),
                Some((0, rest)) => {
                    self.data = rest;
                    visitor.visit_none()
                },
                Some((1, rest)) => {
                    self.data = rest;
                    visitor.visit_some(self)
                }
                Some((_, rest)) => {
                    self.data = rest;
                    Err(DeserializeError::InvalidData)
                }
            }
        }

        fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            visitor.visit_unit()
        }

        fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            visitor.visit_unit()
        }

        fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            visitor.visit_newtype_struct(self)
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            let size = self.get_usize()?;
            struct SeqAccess<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> {
                elements: usize,
                deserializer: &'a mut AglioDeserializer<'de, 'b, Size, W, I>,
            }
            impl<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::de::SeqAccess<'de> for SeqAccess<'a, 'de, 'b, W, I, Size> {
                type Error = DeserializeError;

                fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
                where
                    T: DeserializeSeed<'de>
                {
                    if self.elements == 0 {
                        Ok(None)
                    } else {
                        self.elements = self.elements.saturating_sub(1);
                        seed.deserialize(&mut *self.deserializer).map(Some)
                    }
                }

                fn size_hint(&self) -> Option<usize> {
                    Some(self.elements)
                }
            }

            visitor.visit_seq(SeqAccess{
                elements: usize::from(size),
                deserializer: self,
            })
        }

        fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        { self.deserialize_seq(visitor) }

        fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        { self.deserialize_seq(visitor) }

        fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            todo!()
        }

        fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        { visitor.visit_newtype_struct(self) }

        fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            use serde::de::Deserializer;
            struct VariantAccess<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> {
                deserializer: &'a mut AglioDeserializer<'de, 'b, Size, W, I>,
                variant: &'static str,
            }
            impl<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::de::VariantAccess<'de> for VariantAccess<'a, 'de, 'b, W, I, Size> {
                type Error = DeserializeError;

                fn unit_variant(self) -> Result<(), Self::Error> {
                    Ok(())
                }

                fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
                where
                    T: DeserializeSeed<'de>
                { seed.deserialize(self.deserializer) }

                fn newtype_variant<T>(self) -> Result<T, Self::Error>
                where
                    T: Deserialize<'de>
                { T::deserialize(self.deserializer) }

                fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
                where
                    V: Visitor<'de>
                { self.deserializer.deserialize_tuple(len, visitor) }

                fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
                where
                    V: Visitor<'de>
                { self.deserializer.deserialize_struct(self.variant, fields, visitor) }
            }
            struct EnumAccess<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> {
                deserializer: &'a mut AglioDeserializer<'de, 'b, Size, W, I>,
                variant: &'static str,
            }
            impl<'a, 'de, 'b, W: crc::Width + Clone, I: crc::Implementation<Data<W>: Clone> + Clone, Size: TryFrom<usize> + Serialize + DeserializeOwned + TryInto<usize>> serde::de::EnumAccess<'de> for EnumAccess<'a, 'de, 'b, W, I, Size> {
                type Error = DeserializeError;
                type Variant = VariantAccess<'a, 'de, 'b, W, I, Size>;

                fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
                where
                    V: serde::de::DeserializeSeed<'de>
                {
                    let out = seed.deserialize(&mut *self.deserializer);
                    match out {
                        Err(e) => Err(e),
                        Ok(value) => {
                            let variant = VariantAccess {
                                deserializer: self.deserializer,
                                variant: self.variant,
                            };
                            Ok((value, variant))
                        }
                    }
                }
            }

            let variant_index = self.get_usize()?;;
            visitor.visit_enum(EnumAccess{
                deserializer: self,
                variant: match variants.get(usize::from(variant_index)) {
                    Some(variant) => variant,
                    None => return Err(DeserializeError::InvalidData),
                }
            })
        }

        fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            Err(DeserializeError::NotDescriptive)
        }

        fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>
        {
            Err(DeserializeError::NotDescriptive)
        }

        fn is_human_readable(&self) -> bool {
            false
        }
    }

    //Check Packet start
    let mut data = match data.strip_prefix(config.packet_start) {
        None => return Err(DeserializeError::InvalidPacketStart),
        Some(data) => data,
    };

    let mut deserializer = AglioDeserializer{
        config: config.clone(),
        data,
    };
    //Check & Remove Body size
    let size = deserializer.get_usize()?;
    if size != deserializer.data.len() {
        return Err(DeserializeError::InvalidData)
    }
    //Check & Remove CRC from end of body
    if let Some(crc) = &config.body_crc {
        match data.split_last_chunk() {
            Some((rest, crc_value)) => {
                data = rest;
                let crc_value = match &config.endianess {
                    Endianess::Little => u16::from_le_bytes(*crc_value),
                    Endianess::Big => u16::from_be_bytes(*crc_value),
                };
                if crc.checksum(data) != crc_value {
                    return Err(DeserializeError::ChecksumError);
                }
            }
            None => return Err(DeserializeError::InvalidLength),
        }
    }

    S::deserialize(&mut deserializer)
}