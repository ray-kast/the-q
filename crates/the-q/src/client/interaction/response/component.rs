use serenity::builder::{
    CreateInteractionResponseData, CreateInteractionResponseFollowup, EditInteractionResponse,
};

use super::ResponseData;

#[derive(Debug, Default)]
pub(super) struct Components(Vec<Component>);

macro_rules! build_components {
    ($self:expr, $builder:expr) => {{
        let Self(components) = $self;
        $builder // TODO
    }};
}

impl Components {
    #[inline]
    pub(super) fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_components!(self, res)
    }

    #[inline]
    pub(super) fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_components!(self, fup)
    }
}

impl<'a> ResponseData<'a> for Components {
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        build_components!(self, data)
    }
}

#[derive(Debug, Default)]
pub struct Component {}
