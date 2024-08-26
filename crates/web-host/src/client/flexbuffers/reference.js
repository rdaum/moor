function _class_call_check(instance, Constructor) {
    if (!(instance instanceof Constructor)) {
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
import { fromByteWidth } from './bit-width-util.js';
import { ValueType } from './value-type.js';
import { isNumber, isIndirectNumber, isAVector, fixedTypedVectorElementSize, isFixedTypedVector, isTypedVector, typedVectorElementType, packedType, fixedTypedVectorElementType } from './value-type-util.js';
import { indirect, keyForIndex, keyIndex, readFloat, readInt, readUInt } from './reference-util.js';
import { fromUTF8Array } from './flexbuffers-util.js';
import { BitWidth } from './bit-width.js';
export function toReference(buffer) {
    var len = buffer.byteLength;
    if (len < 3) {
        throw "Buffer needs to be bigger than 3";
    }
    var dataView = new DataView(buffer);
    var byteWidth = dataView.getUint8(len - 1);
    var packedType = dataView.getUint8(len - 2);
    var parentWidth = fromByteWidth(byteWidth);
    var offset = len - byteWidth - 2;
    return new Reference(dataView, offset, parentWidth, packedType, "/");
}
function valueForIndexWithKey(index, key, dataView, offset, parentWidth, byteWidth, length, path) {
    var _indirect = indirect(dataView, offset, parentWidth);
    var elementOffset = _indirect + index * byteWidth;
    var packedType = dataView.getUint8(_indirect + length * byteWidth + index);
    return new Reference(dataView, elementOffset, fromByteWidth(byteWidth), packedType, "".concat(path, "/").concat(key));
}
export var Reference = /*#__PURE__*/ function() {
    "use strict";
    function Reference(dataView, offset, parentWidth, packedType, path) {
        _class_call_check(this, Reference);
        _define_property(this, "dataView", void 0);
        _define_property(this, "offset", void 0);
        _define_property(this, "parentWidth", void 0);
        _define_property(this, "packedType", void 0);
        _define_property(this, "path", void 0);
        _define_property(this, "byteWidth", void 0);
        _define_property(this, "valueType", void 0);
        _define_property(this, "_length", void 0);
        this.dataView = dataView;
        this.offset = offset;
        this.parentWidth = parentWidth;
        this.packedType = packedType;
        this.path = path;
        this._length = -1;
        this.byteWidth = 1 << (packedType & 3);
        this.valueType = packedType >> 2;
    }
    _create_class(Reference, [
        {
            key: "isNull",
            value: function isNull() {
                return this.valueType === ValueType.NULL;
            }
        },
        {
            key: "isNumber",
            value: function isNumber1() {
                return isNumber(this.valueType) || isIndirectNumber(this.valueType);
            }
        },
        {
            key: "isFloat",
            value: function isFloat() {
                return ValueType.FLOAT === this.valueType || ValueType.INDIRECT_FLOAT === this.valueType;
            }
        },
        {
            key: "isInt",
            value: function isInt() {
                return this.isNumber() && !this.isFloat();
            }
        },
        {
            key: "isString",
            value: function isString() {
                return ValueType.STRING === this.valueType || ValueType.KEY === this.valueType;
            }
        },
        {
            key: "isBool",
            value: function isBool() {
                return ValueType.BOOL === this.valueType;
            }
        },
        {
            key: "isBlob",
            value: function isBlob() {
                return ValueType.BLOB === this.valueType;
            }
        },
        {
            key: "isVector",
            value: function isVector() {
                return isAVector(this.valueType);
            }
        },
        {
            key: "isMap",
            value: function isMap() {
                return ValueType.MAP === this.valueType;
            }
        },
        {
            key: "boolValue",
            value: function boolValue() {
                if (this.isBool()) {
                    return readInt(this.dataView, this.offset, this.parentWidth) > 0;
                }
                return null;
            }
        },
        {
            key: "intValue",
            value: function intValue() {
                if (this.valueType === ValueType.INT) {
                    return readInt(this.dataView, this.offset, this.parentWidth);
                }
                if (this.valueType === ValueType.UINT) {
                    return readUInt(this.dataView, this.offset, this.parentWidth);
                }
                if (this.valueType === ValueType.INDIRECT_INT) {
                    return readInt(this.dataView, indirect(this.dataView, this.offset, this.parentWidth), fromByteWidth(this.byteWidth));
                }
                if (this.valueType === ValueType.INDIRECT_UINT) {
                    return readUInt(this.dataView, indirect(this.dataView, this.offset, this.parentWidth), fromByteWidth(this.byteWidth));
                }
                return null;
            }
        },
        {
            key: "floatValue",
            value: function floatValue() {
                if (this.valueType === ValueType.FLOAT) {
                    return readFloat(this.dataView, this.offset, this.parentWidth);
                }
                if (this.valueType === ValueType.INDIRECT_FLOAT) {
                    return readFloat(this.dataView, indirect(this.dataView, this.offset, this.parentWidth), fromByteWidth(this.byteWidth));
                }
                return null;
            }
        },
        {
            key: "numericValue",
            value: function numericValue() {
                return this.floatValue() || this.intValue();
            }
        },
        {
            key: "stringValue",
            value: function stringValue() {
                if (this.valueType === ValueType.STRING || this.valueType === ValueType.KEY) {
                    var begin = indirect(this.dataView, this.offset, this.parentWidth);
                    return fromUTF8Array(new Uint8Array(this.dataView.buffer, begin, this.length()));
                }
                return null;
            }
        },
        {
            key: "blobValue",
            value: function blobValue() {
                if (this.isBlob()) {
                    var begin = indirect(this.dataView, this.offset, this.parentWidth);
                    return new Uint8Array(this.dataView.buffer, begin, this.length());
                }
                return null;
            }
        },
        {
            key: "get",
            value: function get(key) {
                var length = this.length();
                if (Number.isInteger(key) && isAVector(this.valueType)) {
                    if (key >= length || key < 0) {
                        throw "Key: [".concat(key, "] is not applicable on ").concat(this.path, " of ").concat(this.valueType, " length: ").concat(length);
                    }
                    var _indirect = indirect(this.dataView, this.offset, this.parentWidth);
                    var elementOffset = _indirect + key * this.byteWidth;
                    var _packedType = this.dataView.getUint8(_indirect + length * this.byteWidth + key);
                    if (isTypedVector(this.valueType)) {
                        var _valueType = typedVectorElementType(this.valueType);
                        _packedType = packedType(_valueType, BitWidth.WIDTH8);
                    } else if (isFixedTypedVector(this.valueType)) {
                        var _valueType1 = fixedTypedVectorElementType(this.valueType);
                        _packedType = packedType(_valueType1, BitWidth.WIDTH8);
                    }
                    return new Reference(this.dataView, elementOffset, fromByteWidth(this.byteWidth), _packedType, "".concat(this.path, "[").concat(key, "]"));
                }
                if (typeof key === 'string') {
                    var index = keyIndex(key, this.dataView, this.offset, this.parentWidth, this.byteWidth, length);
                    if (index !== null) {
                        return valueForIndexWithKey(index, key, this.dataView, this.offset, this.parentWidth, this.byteWidth, length, this.path);
                    }
                }
                throw "Key [".concat(key, "] is not applicable on ").concat(this.path, " of ").concat(this.valueType);
            }
        },
        {
            key: "length",
            value: function length() {
                var size;
                if (this._length > -1) {
                    return this._length;
                }
                if (isFixedTypedVector(this.valueType)) {
                    this._length = fixedTypedVectorElementSize(this.valueType);
                } else if (this.valueType === ValueType.BLOB || this.valueType === ValueType.MAP || isAVector(this.valueType)) {
                    this._length = readUInt(this.dataView, indirect(this.dataView, this.offset, this.parentWidth) - this.byteWidth, fromByteWidth(this.byteWidth));
                } else if (this.valueType === ValueType.NULL) {
                    this._length = 0;
                } else if (this.valueType === ValueType.STRING) {
                    var _indirect = indirect(this.dataView, this.offset, this.parentWidth);
                    var sizeByteWidth = this.byteWidth;
                    size = readUInt(this.dataView, _indirect - sizeByteWidth, fromByteWidth(this.byteWidth));
                    while(this.dataView.getInt8(_indirect + size) !== 0){
                        sizeByteWidth <<= 1;
                        size = readUInt(this.dataView, _indirect - sizeByteWidth, fromByteWidth(this.byteWidth));
                    }
                    this._length = size;
                } else if (this.valueType === ValueType.KEY) {
                    var _indirect1 = indirect(this.dataView, this.offset, this.parentWidth);
                    size = 1;
                    while(this.dataView.getInt8(_indirect1 + size) !== 0){
                        size++;
                    }
                    this._length = size;
                } else {
                    this._length = 1;
                }
                return Number(this._length);
            }
        },
        {
            key: "toObject",
            value: function toObject() {
                var length = this.length();
                if (this.isVector()) {
                    var result = [];
                    for(var i = 0; i < length; i++){
                        result.push(this.get(i).toObject());
                    }
                    return result;
                }
                if (this.isMap()) {
                    var result1 = {};
                    for(var i1 = 0; i1 < length; i1++){
                        var key = keyForIndex(i1, this.dataView, this.offset, this.parentWidth, this.byteWidth);
                        result1[key] = valueForIndexWithKey(i1, key, this.dataView, this.offset, this.parentWidth, this.byteWidth, length, this.path).toObject();
                    }
                    return result1;
                }
                if (this.isNull()) {
                    return null;
                }
                if (this.isBool()) {
                    return this.boolValue();
                }
                if (this.isNumber()) {
                    return this.numericValue();
                }
                return this.blobValue() || this.stringValue();
            }
        }
    ]);
    return Reference;
}();
