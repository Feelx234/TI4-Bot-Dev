// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * One attachment of a planet, with what it adds.
 */
export type AttachmentRecord = { id: string, 
/**
 * The id where the corpus has no name.
 */
name: string, resources_modifier: number, influence_modifier: number, 
/**
 * The technology specialties the attachment gives (a research facility), lower-case.
 */
tech_specialties?: Array<string>, 
/**
 * The printed text of the card that puts it on a planet (display only).
 */
text?: string, };
