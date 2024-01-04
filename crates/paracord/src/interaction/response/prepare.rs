/// Trait for purging errors from an object
pub trait Prepare {
    /// The resulting error-free type
    type Output;
    /// The error transposed out of `self`
    type Error;

    // TODO: this is becoming a performance nightmare, methinks...

    /// Return any latent errors within `self`, else return a type-safe
    /// error-free version of `self`
    ///
    /// # Errors
    /// This function should return an error if `self` contains any errors
    fn prepare(self) -> Result<Self::Output, Self::Error>;
}
