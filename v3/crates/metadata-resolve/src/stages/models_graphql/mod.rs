mod filter;
mod graphql;
mod types;

use crate::Warning;
use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet};

use lang_graphql::ast::common as ast;
use open_dds::{models::ModelName, types::CustomTypeName};

use crate::helpers::types::TrackGraphQLRootFields;
use crate::stages::{boolean_expressions, graphql_config, models, object_relationships};
use crate::types::error::Error;
use crate::types::subgraph::Qualified;

pub(crate) use types::ModelWithGraphql;
pub use types::{
    ModelGraphQlApi, ModelGraphqlIssue, ModelOrderByExpression, ModelsWithGraphqlOutput,
    SelectAggregateGraphQlDefinition, SelectManyGraphQlDefinition, SelectUniqueGraphQlDefinition,
    SubscriptionGraphQlDefinition, UniqueIdentifierField,
};

use super::order_by_expressions;

pub fn resolve(
    metadata_accessor: &open_dds::accessor::MetadataAccessor,
    models: &IndexMap<Qualified<ModelName>, models::Model>,
    object_types: &BTreeMap<
        Qualified<CustomTypeName>,
        object_relationships::ObjectTypeWithRelationships,
    >,
    boolean_expression_types: &boolean_expressions::BooleanExpressionTypes,
    order_by_expressions: &order_by_expressions::OrderByExpressions,
    existing_graphql_types: &BTreeSet<ast::TypeName>,
    track_root_fields: &mut TrackGraphQLRootFields,
    graphql_config: &graphql_config::GraphqlConfig,
) -> Result<ModelsWithGraphqlOutput, Error> {
    let mut output = ModelsWithGraphqlOutput {
        models_with_graphql: IndexMap::new(),
        issues: vec![],
    };

    // Used to ensure we don't resolve the same type twice.
    let mut existing_graphql_types = existing_graphql_types.clone();

    for (model_name, model) in models.clone() {
        let filter_expression_type = match &model.raw.filter_expression_type {
            Some(filter_expression_type_name) => {
                // We can only create a boolean expression if a source is attached to a model,
                // throw an error if this is not the case
                let model_source = match model.source {
                    Some(ref source) => Ok(source),
                    None => Err(Error::CannotUseFilterExpressionsWithoutSource {
                        model: model.name.clone(),
                    }),
                }?;

                let (filter_expression_type, filter_issues) =
                    filter::resolve_filter_expression_type(
                        &model.name,
                        &model.data_type,
                        model_source,
                        filter_expression_type_name,
                        boolean_expression_types,
                        object_types,
                        models,
                        &metadata_accessor.flags,
                    )?;

                output
                    .issues
                    .extend(filter_issues.into_iter().map(Warning::from));

                Some(filter_expression_type)
            }
            None => None,
        };

        let graphql_api = match model.raw.graphql {
            Some(ref model_graphql_definition) => graphql::resolve_model_graphql_api(
                metadata_accessor,
                model_graphql_definition,
                &model,
                &mut existing_graphql_types,
                track_root_fields,
                model.raw.description.as_ref(),
                model.aggregate_expression.as_ref(),
                order_by_expressions,
                graphql_config,
                &mut output.issues,
            )?,
            None => types::ModelGraphQlApi::default(),
        };

        output.models_with_graphql.insert(
            model_name,
            types::ModelWithGraphql {
                inner: model,
                graphql_api,
                filter_expression_type,
            },
        );
    }

    Ok(output)
}
