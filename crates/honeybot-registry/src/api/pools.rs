//! Pool CRUD and membership.
//!
//! - `POST   /pools`                       create
//! - `GET    /pools/{id}`                  read (members, basic info)
//! - `POST   /pools/join`                  join via invite_code
//! - `DELETE /pools/{id}/members/{guild}`  owner-only revoke
