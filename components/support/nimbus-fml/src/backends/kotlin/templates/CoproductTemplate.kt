{%- let inner = self.inner() %}
{%- let class_name = inner.name()|class_name %}
{%- let discriminant = inner.discriminant()|quoted %}

{{ inner.doc()|comment("") }}
public sealed interface {{ class_name }} : FMLObjectInterface {
    {%- for variant in self.variants() %}

    {{ variant.doc|comment("    ") }}
    public data class {{ variant.class_name }}(val value: {{ variant.object_class_name }}) : {{ class_name }} {
        override fun toJSONObject(): JSONObject =
            value.toJSONObject().put({{ discriminant }}, {{ variant.tag|quoted }})
    }
    {%- endfor %}

    companion object {
        internal fun create(variables: Variables): {{ class_name }}? =
            when (variables.getString({{ discriminant }}) ?: {{ inner.default_variant()|quoted }}) {
                {%- for variant in self.variants() %}
                {{ variant.tag|quoted }} -> {{ variant.object_class_name }}.create(variables)?.let { {{ variant.class_name }}(it) }
                {%- endfor %}
                else -> null
            }

        internal fun mergeWith(overrides: {{ class_name }}, defaults: {{ class_name }}?): {{ class_name }} =
            when {
                {%- for variant in self.variants() %}
                overrides is {{ variant.class_name }} && defaults is {{ variant.class_name }} ->
                    {{ variant.class_name }}(overrides.value._mergeWith(defaults.value))
                {%- endfor %}
                else -> overrides
            }
    }
}
