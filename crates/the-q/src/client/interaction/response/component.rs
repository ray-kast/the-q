use serenity::builder::{
    CreateInteractionResponseData, CreateInteractionResponseFollowup, EditInteractionResponse,
};

use super::ResponseData;

macro_rules! build_components {
    ($self:expr, $builder:expr) => {{
        let Self {} = $self;
        $builder
    }};
}

#[derive(Debug, Default)]
pub struct Components {}

impl Components {
    #[inline]
    pub fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_components!(self, res)
    }

    #[inline]
    pub fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_components!(self, fup)
    }
}

impl ResponseData for Components {
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        build_components!(self, data)
    }
}
