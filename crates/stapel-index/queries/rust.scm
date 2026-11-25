(struct_item name: (type_identifier) @name) @struct
(enum_item name: (type_identifier) @name) @enum
(union_item name: (type_identifier) @name) @union
(trait_item name: (type_identifier) @name) @trait
(type_item name: (type_identifier) @name) @type
(associated_type name: (type_identifier) @name) @type
(const_item name: (identifier) @name) @const
(static_item name: (identifier) @name) @static
(mod_item name: (identifier) @name) @mod
(macro_definition name: (identifier) @name) @macro
(function_item name: (identifier) @name) @fn
(function_signature_item name: (identifier) @name) @fn
(impl_item body: (declaration_list (function_item name: (identifier) @name) @method))
(trait_item body: (declaration_list (function_item name: (identifier) @name) @method))
(trait_item body: (declaration_list (function_signature_item name: (identifier) @name) @method))
