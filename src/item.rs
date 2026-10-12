//! Item records: 19-byte slots (inventories hold 40, from record +6E),
//! the one layout shared by loot, inventory and combat. `?` marks
//! guesses.

/// Slot size in bytes.
pub const SIZE: u16 = 0x13;

/// Weapon, armour or other type index (sign-extended by the name code).
pub const TYPE: u16 = 0x00;
/// The same byte: a stack's count, or for made weapons and armour the
/// condition code from the type tables (see `loot::set_condition`).
pub const COUNT: u16 = 0x01;
pub const CONDITION_CODE: u16 = 0x01;
/// Weapons: hands needed (1 or 2).
pub const HANDS: u16 = 0x03;
/// Weapons: damage min and max bytes; armour: the rating (byte 4).
pub const DAMAGE_MIN: u16 = 0x04;
pub const DAMAGE_MAX: u16 = 0x05;
pub const RATING: u16 = 0x04;
/// Condition (word), worn down by use; 0 is broken; and its maximum.
pub const HEALTH: u16 = 0x06;
pub const MAX_HEALTH: u16 = 0x08;
/// Price (dword).
pub const PRICE: u16 = 0x0A;
/// Flags (byte): see the bits below.
pub const FLAGS: u16 = 0x0E;
/// Enchantment: the "of ..." suffix index (0xFF none).
pub const ENCHANT: u16 = 0x0F;
/// Material index (0 Iron .. 7 Ebony), the armour kind for plain armour,
/// or 0xFF; for accessories the ring suffix list or material.
pub const MATERIAL: u16 = 0x10;
pub const ENCHANT_VALUE: u16 = 0x11; // ?
/// The attribute (or spell?) the enchantment affects.
pub const ENCHANT_ATTR: u16 = 0x12;
pub const ENCHANT_SPELL: u16 = 0x12; // ?

/// FLAGS bits: enchanted (also "has a material"), not yet identified,
/// the category (`(flags & 0x0C) >> 2`: 0 armour, 1 weapon, 2
/// accessory, 3 stack), the kind or material list (`(flags & 0x30) >>
/// 4`), slot occupied, equipped.
pub const ENCHANTED: u8 = 0x01;
pub const UNIDENTIFIED: u8 = 0x02;
pub const CATEGORY: u8 = 0x0C;
pub const KIND: u8 = 0x30;
pub const OCCUPIED: u8 = 0x40;
pub const EQUIPPED: u8 = 0x80;
