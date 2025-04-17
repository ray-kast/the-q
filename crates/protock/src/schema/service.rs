use std::collections::HashMap;

use prost_types::method_options::IdempotencyLevel;

use super::{
    qual_name::{MemberQualName, QualName},
    ty::TypeContext,
    TypeError,
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
    compat_pair::{CompatPair, Side, Variance},
};

pub struct MethodContext<'a> {
    ty: &'a TypeContext<'a>,
    name: &'a str,
}

fn check_ty<V: Variance>(
    pair: CompatPair<(&MethodContext<'_>, &QualName<'static>), V>,
    log: &mut CompatLog,
    err: impl FnOnce(Side<TypeError<'_, QualName<'_>>>, &mut CompatLog),
) {
    match pair.try_map(|(c, t)| c.ty.types.get(t).map(|t| (c, t))) {
        Ok(c) => {
            let (cx, ty) = c.unzip();
            ty.check(cx.map(|c| c.ty.clone()), log);
        },
        Err(e) => err(e, log),
    }
}

fn check_stream<V: Variance>(stream: CompatPair<bool, V>, err: impl FnOnce(CompatPair<bool, V>)) {
    match stream.into_inner() {
        (rd, wr) if rd == wr => (),
        (true, false) => (),
        _ => err(stream),
    }
}

#[derive(Debug)]
pub struct Method {
    pub idempotency: IdempotencyLevel,
    pub deprecated: bool,
    pub input_type: QualName<'static>,
    pub input_stream: bool,
    pub output_type: QualName<'static>,
    pub output_stream: bool,
}

impl CheckCompat for Method {
    type Context<'a> = MethodContext<'a>;

    fn check_compat<V: Variance>(
        ck: CompatPair<&'_ Self, V>,
        cx: CompatPair<Self::Context<'_>, V>,
        log: &mut CompatLog,
    ) {
        let ck = ck.map(
            |Method {
                 idempotency,
                 deprecated: _,
                 input_type,
                 input_stream,
                 output_type,
                 output_stream,
             }| {
                (
                    (((idempotency, input_type), input_stream), output_type),
                    output_stream,
                )
            },
        );
        let (ck, output_stream) = ck.unzip();
        let (ck, output_type) = ck.unzip();
        let (ck, input_stream) = ck.unzip();
        let (idempotency, input_type) = ck.unzip();

        // A note on terminology: since compat_pair uses reader/writer, operate
        // under the assumption that reader => server and writer => client

        let qual_names = cx.as_ref().map(|c| c.ty.kind.type_name().member(c.name));

        check_ty(cx.as_ref().zip(input_type), log, |e, log| {
            CompatError::new(
                e.kind()
                    .project(qual_names.as_ref())
                    .map(MemberQualName::to_owned)
                    .into(),
                format!(
                    "Input type resolution failure for {:?}",
                    e.map(|e| e.0).display()
                ),
            )
            .err(log);
        });

        check_ty(cx.as_ref().zip(output_type).flip(), log, |e, log| {
            CompatError::new(
                e.kind()
                    .project(qual_names.as_ref().flip())
                    .map(MemberQualName::to_owned)
                    .into(),
                format!(
                    "Outut type resolution failure for {:?}",
                    e.map(|e| e.0).display()
                ),
            )
            .err(log);
        });

        match idempotency.into_inner() {
            (rd, wr) if rd == wr => (),
            (_, IdempotencyLevel::IdempotencyUnknown)
            | (IdempotencyLevel::NoSideEffects, IdempotencyLevel::Idempotent) => (),
            _ => CompatError::new_var(
                qual_names.as_ref().map(MemberQualName::to_owned).into(),
                format!(
                    "Idempotency mismatch ({})",
                    idempotency.map(IdempotencyLevel::as_str_name).display()
                ),
            )
            .err(log),
        }

        check_stream(input_stream.copied(), |s| {
            CompatError::new_var(
                qual_names.as_ref().map(MemberQualName::to_owned).into(),
                format!("Input streaming mismatch ({})", s.display()),
            )
            .err(log);
        });

        check_stream(output_stream.copied().flip(), |s| {
            CompatError::new_var(
                qual_names
                    .as_ref()
                    .flip()
                    .map(MemberQualName::to_owned)
                    .into(),
                format!("Output streaming mismatch ({})", s.display()),
            )
            .err(log);
        });
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Service(HashMap<String, Method>);

impl Service {
    pub fn new(methods: HashMap<String, Method>) -> Self { Self(methods) }
}

impl CheckCompat for Service {
    type Context<'a> = TypeContext<'a>;

    fn check_compat<V: Variance>(
        ck: CompatPair<&'_ Self, V>,
        cx: CompatPair<Self::Context<'_>, V>,
        log: &mut CompatLog,
    ) {
        // A note on terminology: since compat_pair uses reader/writer, operate
        // under the assumption that reader => server and writer => client

        ck.map(|Service(methods)| methods).check_joined(
            &cx,
            log,
            |ty, name| MethodContext { ty, name },
            |name, method, log| {
                if !method.map(|m| m.deprecated).inner() {
                    CompatError::new_var(
                        cx.as_ref().map(|c| c.kind.to_owned()).into(),
                        format!(
                            "Method {name} missing and not deprecated on {}",
                            method.kind().opposite().pretty()
                        ),
                    )
                    .warn(log);
                }
            },
        );
    }
}
