use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Property {
    pub id: Uuid,
    pub name: String,
    pub custom_cards: Vec<CustomCard>,
    pub is_protected: bool,
    pub is_public: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCard {
    pub event: String,
    #[serde(default)]
    pub value: bool,
}

#[derive(Debug, FromRow)]
pub struct PropertyRow {
    pub id: Vec<u8>,
    pub name: String,
    pub custom_cards: String,
    pub is_protected: i64,
    pub is_public: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl PropertyRow {
    pub fn into_property(self) -> Property {
        let id = Uuid::from_slice(&self.id).unwrap_or_default();
        let custom_cards: Vec<CustomCard> =
            serde_json::from_str(&self.custom_cards).unwrap_or_default();
        Property {
            id,
            name: self.name,
            custom_cards,
            is_protected: self.is_protected != 0,
            is_public: self.is_public != 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
