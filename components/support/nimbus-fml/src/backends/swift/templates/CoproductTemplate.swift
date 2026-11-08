{%- let inner = self.inner() %}
{%- let class_name = inner.name()|class_name %}
{%- let discriminant = inner.discriminant()|quoted %}

{{ inner.doc()|comment("") }}
public enum {{ class_name }}: FMLObjectInterface {
    {%- for variant in self.variants() %}
    {{ variant.doc|comment("    ") }}
    case {{ variant.case_name }}({{ variant.object_class_name }})
    {%- endfor %}

    private enum DiscriminantKey: String, CodingKey {
        case discriminant = {{ discriminant }}
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: DiscriminantKey.self)
        switch self {
        {%- for variant in self.variants() %}
        case let .{{ variant.case_name }}(value):
            try value.encode(to: encoder)
            try container.encode({{ variant.tag|quoted }}, forKey: .discriminant)
        {%- endfor %}
        }
    }
}

public extension {{ class_name }} {
    func _mergeWith(_ defaults: {{ class_name }}?) -> {{ class_name }} {
        switch (self, defaults) {
        {%- for variant in self.variants() %}
        case let (.{{ variant.case_name }}(overrides), .{{ variant.case_name }}(defaults)?):
            return .{{ variant.case_name }}(overrides._mergeWith(defaults))
        {%- endfor %}
        default:
            return self
        }
    }

    static func create(_ variables: Variables?) -> {{ class_name }}? {
        let variables = variables ?? NilVariables.instance
        switch variables.getString({{ discriminant }}) ?? {{ inner.default_variant()|quoted }} {
        {%- for variant in self.variants() %}
        case {{ variant.tag|quoted }}:
            return .{{ variant.case_name }}({{ variant.object_class_name }}.create(variables))
        {%- endfor %}
        default:
            return nil
        }
    }

    static func mergeWith(_ overrides: {{ class_name }}, _ defaults: {{ class_name }}) -> {{ class_name }} {
        return overrides._mergeWith(defaults)
    }
}
