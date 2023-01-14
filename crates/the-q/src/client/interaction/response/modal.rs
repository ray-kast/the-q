use serenity::builder::CreateInteractionResponseData;

use super::ResponseData;

pub struct Modal; // TODO

impl ResponseData for Modal {
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        todo!("{data:?}")
    }
}
