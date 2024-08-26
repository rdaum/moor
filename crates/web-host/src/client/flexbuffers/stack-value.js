function _instanceof(left, right) {
    if (right != null && typeof Symbol !== "undefined" && right[Symbol.hasInstance]) {
        return !!right[Symbol.hasInstance](left);
    } else {
        return left instanceof right;
    }
}
function _class_call_check(instance, Constructor) {
    if (!_instanceof(instance, Constructor)) {
        throw new TypeError("Cannot call a class as a function");
    }
}
function _defineProperties(target, props) {
    for(var i = 0; i < props.length; i++){
        var descriptor = props[i];
        descriptor.enumerable = descriptor.enumerable || false;
        descriptor.configurable = true;
        if ("value" in descriptor) descriptor.writable = true;
        Object.defineProperty(target, descriptor.key, descriptor);
    }
}
function _create_class(Constructor, protoProps, staticProps) {
    if (protoProps) _defineProperties(Constructor.prototype, protoProps);
    if (staticProps) _defineProperties(Constructor, staticProps);
    return Constructor;
}
function _define_property(obj, key, value) {
    if (key in obj) {
        Object.defineProperty(obj, key, {
            value: value,
            enumerable: true,
            configurable: true,
            writable: true
        });
    } else {
        obj[key] = value;
    }
    return obj;
}
import { BitWidth } from './bit-width.js';
import { paddingSize, uwidth, fromByteWidth } from './bit-width-util.js';
import { ValueType } from './value-type.js';
import { isInline, packedType } from './value-type-util.js';
export var StackValue = /*#__PURE__*/ function() {
    "use strict";
    function StackValue(builder, type, width) {
        var value = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : null, offset = arguments.length > 4 && arguments[4] !== void 0 ? arguments[4] : 0;
        _class_call_check(this, StackValue);
        _define_property(this, "builder", void 0);
        _define_property(this, "type", void 0);
        _define_property(this, "width", void 0);
        _define_property(this, "value", void 0);
        _define_property(this, "offset", void 0);
        this.builder = builder;
        this.type = type;
        this.width = width;
        this.value = value;
        this.offset = offset;
    }
    _create_class(StackValue, [
        {
            key: "elementWidth",
            value: function elementWidth(size, index) {
                if (isInline(this.type)) return this.width;
                for(var i = 0; i < 4; i++){
                    var width = 1 << i;
                    var offsetLoc = size + paddingSize(size, width) + index * width;
                    var offset = offsetLoc - this.offset;
                    var bitWidth = uwidth(offset);
                    if (1 << bitWidth === width) {
                        return bitWidth;
                    }
                }
                throw "Element is unknown. Size: ".concat(size, " at index: ").concat(index, ". This might be a bug. Please create an issue https://github.com/google/flatbuffers/issues/new");
            }
        },
        {
            key: "writeToBuffer",
            value: function writeToBuffer(byteWidth) {
                var newOffset = this.builder.computeOffset(byteWidth);
                if (this.type === ValueType.FLOAT) {
                    if (this.width === BitWidth.WIDTH32) {
                        this.builder.view.setFloat32(this.builder.offset, this.value, true);
                    } else {
                        this.builder.view.setFloat64(this.builder.offset, this.value, true);
                    }
                } else if (this.type === ValueType.INT) {
                    var bitWidth = fromByteWidth(byteWidth);
                    this.builder.pushInt(this.value, bitWidth);
                } else if (this.type === ValueType.UINT) {
                    var bitWidth1 = fromByteWidth(byteWidth);
                    this.builder.pushUInt(this.value, bitWidth1);
                } else if (this.type === ValueType.NULL) {
                    this.builder.pushInt(0, this.width);
                } else if (this.type === ValueType.BOOL) {
                    this.builder.pushInt(this.value ? 1 : 0, this.width);
                } else {
                    throw "Unexpected type: ".concat(this.type, ". This might be a bug. Please create an issue https://github.com/google/flatbuffers/issues/new");
                }
                this.offset = newOffset;
            }
        },
        {
            key: "storedWidth",
            value: function storedWidth() {
                var width = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : BitWidth.WIDTH8;
                return isInline(this.type) ? Math.max(width, this.width) : this.width;
            }
        },
        {
            key: "storedPackedType",
            value: function storedPackedType() {
                var width = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : BitWidth.WIDTH8;
                return packedType(this.type, this.storedWidth(width));
            }
        },
        {
            key: "isOffset",
            value: function isOffset() {
                return !isInline(this.type);
            }
        }
    ]);
    return StackValue;
}();
