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
function _instanceof1(left, right) {
    if (right != null && typeof Symbol !== "undefined" && right[Symbol.hasInstance]) {
        return !!right[Symbol.hasInstance](left);
    } else {
        return _instanceof(left, right);
    }
}
function _type_of(obj) {
    "@swc/helpers - typeof";
    return obj && typeof Symbol !== "undefined" && obj.constructor === Symbol ? "symbol" : typeof obj;
}
import { BitWidth } from './bit-width.js';
import { paddingSize, iwidth, uwidth, fwidth, toByteWidth, fromByteWidth } from './bit-width-util.js';
import { toUTF8Array } from './flexbuffers-util.js';
import { ValueType } from './value-type.js';
import { isNumber, isTypedVectorElement, toTypedVector } from './value-type-util.js';
import { StackValue } from './stack-value.js';
export var Builder = /*#__PURE__*/ function() {
    "use strict";
    function Builder() {
        var size = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : 2048, dedupStrings = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : true, dedupKeys = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : true, dedupKeyVectors = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : true;
        _class_call_check(this, Builder);
        _define_property(this, "dedupStrings", void 0);
        _define_property(this, "dedupKeys", void 0);
        _define_property(this, "dedupKeyVectors", void 0);
        _define_property(this, "buffer", void 0);
        _define_property(this, "view", void 0);
        _define_property(this, "stack", void 0);
        _define_property(this, "stackPointers", void 0);
        _define_property(this, "offset", void 0);
        _define_property(this, "finished", void 0);
        _define_property(this, "stringLookup", void 0);
        _define_property(this, "keyLookup", void 0);
        _define_property(this, "keyVectorLookup", void 0);
        _define_property(this, "indirectIntLookup", void 0);
        _define_property(this, "indirectUIntLookup", void 0);
        _define_property(this, "indirectFloatLookup", void 0);
        this.dedupStrings = dedupStrings;
        this.dedupKeys = dedupKeys;
        this.dedupKeyVectors = dedupKeyVectors;
        this.stack = [];
        this.stackPointers = [];
        this.offset = 0;
        this.finished = false;
        this.stringLookup = {};
        this.keyLookup = {};
        this.keyVectorLookup = {};
        this.indirectIntLookup = {};
        this.indirectUIntLookup = {};
        this.indirectFloatLookup = {};
        this.buffer = new ArrayBuffer(size > 0 ? size : 2048);
        this.view = new DataView(this.buffer);
    }
    _create_class(Builder, [
        {
            key: "align",
            value: function align(width) {
                var byteWidth = toByteWidth(width);
                this.offset += paddingSize(this.offset, byteWidth);
                return byteWidth;
            }
        },
        {
            key: "computeOffset",
            value: function computeOffset(newValueSize) {
                var targetOffset = this.offset + newValueSize;
                var size = this.buffer.byteLength;
                var prevSize = size;
                while(size < targetOffset){
                    size <<= 1;
                }
                if (prevSize < size) {
                    var prevBuffer = this.buffer;
                    this.buffer = new ArrayBuffer(size);
                    this.view = new DataView(this.buffer);
                    new Uint8Array(this.buffer).set(new Uint8Array(prevBuffer), 0);
                }
                return targetOffset;
            }
        },
        {
            key: "pushInt",
            value: function pushInt(value, width) {
                if (width === BitWidth.WIDTH8) {
                    this.view.setInt8(this.offset, value);
                } else if (width === BitWidth.WIDTH16) {
                    this.view.setInt16(this.offset, value, true);
                } else if (width === BitWidth.WIDTH32) {
                    this.view.setInt32(this.offset, value, true);
                } else if (width === BitWidth.WIDTH64) {
                    this.view.setBigInt64(this.offset, BigInt(value), true);
                } else {
                    throw "Unexpected width: ".concat(width, " for value: ").concat(value);
                }
            }
        },
        {
            key: "pushUInt",
            value: function pushUInt(value, width) {
                if (width === BitWidth.WIDTH8) {
                    this.view.setUint8(this.offset, value);
                } else if (width === BitWidth.WIDTH16) {
                    this.view.setUint16(this.offset, value, true);
                } else if (width === BitWidth.WIDTH32) {
                    this.view.setUint32(this.offset, value, true);
                } else if (width === BitWidth.WIDTH64) {
                    this.view.setBigUint64(this.offset, BigInt(value), true);
                } else {
                    throw "Unexpected width: ".concat(width, " for value: ").concat(value);
                }
            }
        },
        {
            key: "writeInt",
            value: function writeInt(value, byteWidth) {
                var newOffset = this.computeOffset(byteWidth);
                this.pushInt(value, fromByteWidth(byteWidth));
                this.offset = newOffset;
            }
        },
        {
            key: "writeUInt",
            value: function writeUInt(value, byteWidth) {
                var newOffset = this.computeOffset(byteWidth);
                this.pushUInt(value, fromByteWidth(byteWidth));
                this.offset = newOffset;
            }
        },
        {
            key: "writeBlob",
            value: function writeBlob(arrayBuffer) {
                var length = arrayBuffer.byteLength;
                var bitWidth = uwidth(length);
                var byteWidth = this.align(bitWidth);
                this.writeUInt(length, byteWidth);
                var blobOffset = this.offset;
                var newOffset = this.computeOffset(length);
                new Uint8Array(this.buffer).set(new Uint8Array(arrayBuffer), blobOffset);
                this.stack.push(this.offsetStackValue(blobOffset, ValueType.BLOB, bitWidth));
                this.offset = newOffset;
            }
        },
        {
            key: "writeString",
            value: function writeString(str) {
                if (this.dedupStrings && Object.prototype.hasOwnProperty.call(this.stringLookup, str)) {
                    this.stack.push(this.stringLookup[str]);
                    return;
                }
                var utf8 = toUTF8Array(str);
                var length = utf8.length;
                var bitWidth = uwidth(length);
                var byteWidth = this.align(bitWidth);
                this.writeUInt(length, byteWidth);
                var stringOffset = this.offset;
                var newOffset = this.computeOffset(length + 1);
                new Uint8Array(this.buffer).set(utf8, stringOffset);
                var stackValue = this.offsetStackValue(stringOffset, ValueType.STRING, bitWidth);
                this.stack.push(stackValue);
                if (this.dedupStrings) {
                    this.stringLookup[str] = stackValue;
                }
                this.offset = newOffset;
            }
        },
        {
            key: "writeKey",
            value: function writeKey(str) {
                if (this.dedupKeys && Object.prototype.hasOwnProperty.call(this.keyLookup, str)) {
                    this.stack.push(this.keyLookup[str]);
                    return;
                }
                var utf8 = toUTF8Array(str);
                var length = utf8.length;
                var newOffset = this.computeOffset(length + 1);
                new Uint8Array(this.buffer).set(utf8, this.offset);
                var stackValue = this.offsetStackValue(this.offset, ValueType.KEY, BitWidth.WIDTH8);
                this.stack.push(stackValue);
                if (this.dedupKeys) {
                    this.keyLookup[str] = stackValue;
                }
                this.offset = newOffset;
            }
        },
        {
            key: "writeStackValue",
            value: function writeStackValue(value, byteWidth) {
                var newOffset = this.computeOffset(byteWidth);
                if (value.isOffset()) {
                    var relativeOffset = this.offset - value.offset;
                    if (byteWidth === 8 || BigInt(relativeOffset) < BigInt(1) << BigInt(byteWidth * 8)) {
                        this.writeUInt(relativeOffset, byteWidth);
                    } else {
                        throw "Unexpected size ".concat(byteWidth, ". This might be a bug. Please create an issue https://github.com/google/flatbuffers/issues/new");
                    }
                } else {
                    value.writeToBuffer(byteWidth);
                }
                this.offset = newOffset;
            }
        },
        {
            key: "integrityCheckOnValueAddition",
            value: function integrityCheckOnValueAddition() {
                if (this.finished) {
                    throw "Adding values after finish is prohibited";
                }
                if (this.stackPointers.length !== 0 && this.stackPointers[this.stackPointers.length - 1].isVector === false) {
                    if (this.stack[this.stack.length - 1].type !== ValueType.KEY) {
                        throw "Adding value to a map before adding a key is prohibited";
                    }
                }
            }
        },
        {
            key: "integrityCheckOnKeyAddition",
            value: function integrityCheckOnKeyAddition() {
                if (this.finished) {
                    throw "Adding values after finish is prohibited";
                }
                if (this.stackPointers.length === 0 || this.stackPointers[this.stackPointers.length - 1].isVector) {
                    throw "Adding key before starting a map is prohibited";
                }
            }
        },
        {
            key: "startVector",
            value: function startVector() {
                this.stackPointers.push({
                    stackPosition: this.stack.length,
                    isVector: true
                });
            }
        },
        {
            key: "startMap",
            value: function startMap() {
                var presorted = arguments.length > 0 && arguments[0] !== void 0 ? arguments[0] : false;
                this.stackPointers.push({
                    stackPosition: this.stack.length,
                    isVector: false,
                    presorted: presorted
                });
            }
        },
        {
            key: "endVector",
            value: function endVector(stackPointer) {
                var vecLength = this.stack.length - stackPointer.stackPosition;
                var vec = this.createVector(stackPointer.stackPosition, vecLength, 1);
                this.stack.splice(stackPointer.stackPosition, vecLength);
                this.stack.push(vec);
            }
        },
        {
            key: "endMap",
            value: function endMap(stackPointer) {
                if (!stackPointer.presorted) {
                    this.sort(stackPointer);
                }
                var keyVectorHash = "";
                for(var i = stackPointer.stackPosition; i < this.stack.length; i += 2){
                    keyVectorHash += ",".concat(this.stack[i].offset);
                }
                var vecLength = this.stack.length - stackPointer.stackPosition >> 1;
                if (this.dedupKeyVectors && !Object.prototype.hasOwnProperty.call(this.keyVectorLookup, keyVectorHash)) {
                    this.keyVectorLookup[keyVectorHash] = this.createVector(stackPointer.stackPosition, vecLength, 2);
                }
                var keysStackValue = this.dedupKeyVectors ? this.keyVectorLookup[keyVectorHash] : this.createVector(stackPointer.stackPosition, vecLength, 2);
                var valuesStackValue = this.createVector(stackPointer.stackPosition + 1, vecLength, 2, keysStackValue);
                this.stack.splice(stackPointer.stackPosition, vecLength << 1);
                this.stack.push(valuesStackValue);
            }
        },
        {
            key: "sort",
            value: function sort(stackPointer) {
                var view = this.view;
                var stack = this.stack;
                function shouldFlip(v1, v2) {
                    if (v1.type !== ValueType.KEY || v2.type !== ValueType.KEY) {
                        throw "Stack values are not keys ".concat(v1, " | ").concat(v2, ". Check if you combined [addKey] with add... method calls properly.");
                    }
                    var c1, c2;
                    var index = 0;
                    do {
                        c1 = view.getUint8(v1.offset + index);
                        c2 = view.getUint8(v2.offset + index);
                        if (c2 < c1) return true;
                        if (c1 < c2) return false;
                        index += 1;
                    }while (c1 !== 0 && c2 !== 0);
                    return false;
                }
                function swap(stack, flipIndex, i) {
                    if (flipIndex === i) return;
                    var k = stack[flipIndex];
                    var v = stack[flipIndex + 1];
                    stack[flipIndex] = stack[i];
                    stack[flipIndex + 1] = stack[i + 1];
                    stack[i] = k;
                    stack[i + 1] = v;
                }
                function selectionSort() {
                    for(var i = stackPointer.stackPosition; i < stack.length; i += 2){
                        var flipIndex = i;
                        for(var j = i + 2; j < stack.length; j += 2){
                            if (shouldFlip(stack[flipIndex], stack[j])) {
                                flipIndex = j;
                            }
                        }
                        if (flipIndex !== i) {
                            swap(stack, flipIndex, i);
                        }
                    }
                }
                function smaller(v1, v2) {
                    if (v1.type !== ValueType.KEY || v2.type !== ValueType.KEY) {
                        throw "Stack values are not keys ".concat(v1, " | ").concat(v2, ". Check if you combined [addKey] with add... method calls properly.");
                    }
                    if (v1.offset === v2.offset) {
                        return false;
                    }
                    var c1, c2;
                    var index = 0;
                    do {
                        c1 = view.getUint8(v1.offset + index);
                        c2 = view.getUint8(v2.offset + index);
                        if (c1 < c2) return true;
                        if (c2 < c1) return false;
                        index += 1;
                    }while (c1 !== 0 && c2 !== 0);
                    return false;
                }
                function quickSort(left, right) {
                    if (left < right) {
                        var mid = left + (right - left >> 2) * 2;
                        var pivot = stack[mid];
                        var left_new = left;
                        var right_new = right;
                        do {
                            while(smaller(stack[left_new], pivot)){
                                left_new += 2;
                            }
                            while(smaller(pivot, stack[right_new])){
                                right_new -= 2;
                            }
                            if (left_new <= right_new) {
                                swap(stack, left_new, right_new);
                                left_new += 2;
                                right_new -= 2;
                            }
                        }while (left_new <= right_new);
                        quickSort(left, right_new);
                        quickSort(left_new, right);
                    }
                }
                var sorted = true;
                for(var i = stackPointer.stackPosition; i < this.stack.length - 2; i += 2){
                    if (shouldFlip(this.stack[i], this.stack[i + 2])) {
                        sorted = false;
                        break;
                    }
                }
                if (!sorted) {
                    if (this.stack.length - stackPointer.stackPosition > 40) {
                        quickSort(stackPointer.stackPosition, this.stack.length - 2);
                    } else {
                        selectionSort();
                    }
                }
            }
        },
        {
            key: "end",
            value: function end() {
                if (this.stackPointers.length < 1) return;
                var pointer = this.stackPointers.pop();
                if (pointer.isVector) {
                    this.endVector(pointer);
                } else {
                    this.endMap(pointer);
                }
            }
        },
        {
            key: "createVector",
            value: function createVector(start, vecLength, step) {
                var keys = arguments.length > 3 && arguments[3] !== void 0 ? arguments[3] : null;
                var bitWidth = uwidth(vecLength);
                var prefixElements = 1;
                if (keys !== null) {
                    var elementWidth = keys.elementWidth(this.offset, 0);
                    if (elementWidth > bitWidth) {
                        bitWidth = elementWidth;
                    }
                    prefixElements += 2;
                }
                var vectorType = ValueType.KEY;
                var typed = keys === null;
                for(var i = start; i < this.stack.length; i += step){
                    var elementWidth1 = this.stack[i].elementWidth(this.offset, i + prefixElements);
                    if (elementWidth1 > bitWidth) {
                        bitWidth = elementWidth1;
                    }
                    if (i === start) {
                        vectorType = this.stack[i].type;
                        typed = typed && isTypedVectorElement(vectorType);
                    } else {
                        if (vectorType !== this.stack[i].type) {
                            typed = false;
                        }
                    }
                }
                var byteWidth = this.align(bitWidth);
                var fix = typed && isNumber(vectorType) && vecLength >= 2 && vecLength <= 4;
                if (keys !== null) {
                    this.writeStackValue(keys, byteWidth);
                    this.writeUInt(1 << keys.width, byteWidth);
                }
                if (!fix) {
                    this.writeUInt(vecLength, byteWidth);
                }
                var vecOffset = this.offset;
                for(var i1 = start; i1 < this.stack.length; i1 += step){
                    this.writeStackValue(this.stack[i1], byteWidth);
                }
                if (!typed) {
                    for(var i2 = start; i2 < this.stack.length; i2 += step){
                        this.writeUInt(this.stack[i2].storedPackedType(), 1);
                    }
                }
                if (keys !== null) {
                    return this.offsetStackValue(vecOffset, ValueType.MAP, bitWidth);
                }
                if (typed) {
                    var vType = toTypedVector(vectorType, fix ? vecLength : 0);
                    return this.offsetStackValue(vecOffset, vType, bitWidth);
                }
                return this.offsetStackValue(vecOffset, ValueType.VECTOR, bitWidth);
            }
        },
        {
            key: "nullStackValue",
            value: function nullStackValue() {
                return new StackValue(this, ValueType.NULL, BitWidth.WIDTH8);
            }
        },
        {
            key: "boolStackValue",
            value: function boolStackValue(value) {
                return new StackValue(this, ValueType.BOOL, BitWidth.WIDTH8, value);
            }
        },
        {
            key: "intStackValue",
            value: function intStackValue(value) {
                return new StackValue(this, ValueType.INT, iwidth(value), value);
            }
        },
        {
            key: "uintStackValue",
            value: function uintStackValue(value) {
                return new StackValue(this, ValueType.UINT, uwidth(value), value);
            }
        },
        {
            key: "floatStackValue",
            value: function floatStackValue(value) {
                return new StackValue(this, ValueType.FLOAT, fwidth(value), value);
            }
        },
        {
            key: "offsetStackValue",
            value: function offsetStackValue(offset, valueType, bitWidth) {
                return new StackValue(this, valueType, bitWidth, null, offset);
            }
        },
        {
            key: "finishBuffer",
            value: function finishBuffer() {
                if (this.stack.length !== 1) {
                    throw "Stack has to be exactly 1, but it is ".concat(this.stack.length, ". You have to end all started vectors and maps before calling [finish]");
                }
                var value = this.stack[0];
                var byteWidth = this.align(value.elementWidth(this.offset, 0));
                this.writeStackValue(value, byteWidth);
                this.writeUInt(value.storedPackedType(), 1);
                this.writeUInt(byteWidth, 1);
                this.finished = true;
            }
        },
        {
            key: "add",
            value: function add(value) {
                this.integrityCheckOnValueAddition();
                if (typeof value === 'undefined') {
                    throw "You need to provide a value";
                }
                if (value === null) {
                    this.stack.push(this.nullStackValue());
                } else if (typeof value === "boolean") {
                    this.stack.push(this.boolStackValue(value));
                } else if ((typeof value === "undefined" ? "undefined" : _type_of(value)) === "bigint") {
                    this.stack.push(this.intStackValue(value));
                } else if (typeof value == 'number') {
                    if (Number.isInteger(value)) {
                        this.stack.push(this.intStackValue(value));
                    } else {
                        this.stack.push(this.floatStackValue(value));
                    }
                } else if (ArrayBuffer.isView(value)) {
                    this.writeBlob(value.buffer);
                } else if (typeof value === 'string' || _instanceof1(value, String)) {
                    this.writeString(value);
                } else if (Array.isArray(value)) {
                    this.startVector();
                    for(var i = 0; i < value.length; i++){
                        this.add(value[i]);
                    }
                    this.end();
                } else if ((typeof value === "undefined" ? "undefined" : _type_of(value)) === 'object') {
                    var properties = Object.getOwnPropertyNames(value).sort();
                    this.startMap(true);
                    for(var i1 = 0; i1 < properties.length; i1++){
                        var key = properties[i1];
                        this.addKey(key);
                        this.add(value[key]);
                    }
                    this.end();
                } else {
                    throw "Unexpected value input ".concat(value);
                }
            }
        },
        {
            key: "finish",
            value: function finish() {
                if (!this.finished) {
                    this.finishBuffer();
                }
                var result = this.buffer.slice(0, this.offset);
                return new Uint8Array(result);
            }
        },
        {
            key: "isFinished",
            value: function isFinished() {
                return this.finished;
            }
        },
        {
            key: "addKey",
            value: function addKey(key) {
                this.integrityCheckOnKeyAddition();
                this.writeKey(key);
            }
        },
        {
            key: "addInt",
            value: function addInt(value) {
                var indirect = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : false, deduplicate = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : false;
                this.integrityCheckOnValueAddition();
                if (!indirect) {
                    this.stack.push(this.intStackValue(value));
                    return;
                }
                if (deduplicate && Object.prototype.hasOwnProperty.call(this.indirectIntLookup, value)) {
                    this.stack.push(this.indirectIntLookup[value]);
                    return;
                }
                var stackValue = this.intStackValue(value);
                var byteWidth = this.align(stackValue.width);
                var newOffset = this.computeOffset(byteWidth);
                var valueOffset = this.offset;
                stackValue.writeToBuffer(byteWidth);
                var stackOffset = this.offsetStackValue(valueOffset, ValueType.INDIRECT_INT, stackValue.width);
                this.stack.push(stackOffset);
                this.offset = newOffset;
                if (deduplicate) {
                    this.indirectIntLookup[value] = stackOffset;
                }
            }
        },
        {
            key: "addUInt",
            value: function addUInt(value) {
                var indirect = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : false, deduplicate = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : false;
                this.integrityCheckOnValueAddition();
                if (!indirect) {
                    this.stack.push(this.uintStackValue(value));
                    return;
                }
                if (deduplicate && Object.prototype.hasOwnProperty.call(this.indirectUIntLookup, value)) {
                    this.stack.push(this.indirectUIntLookup[value]);
                    return;
                }
                var stackValue = this.uintStackValue(value);
                var byteWidth = this.align(stackValue.width);
                var newOffset = this.computeOffset(byteWidth);
                var valueOffset = this.offset;
                stackValue.writeToBuffer(byteWidth);
                var stackOffset = this.offsetStackValue(valueOffset, ValueType.INDIRECT_UINT, stackValue.width);
                this.stack.push(stackOffset);
                this.offset = newOffset;
                if (deduplicate) {
                    this.indirectUIntLookup[value] = stackOffset;
                }
            }
        },
        {
            key: "addFloat",
            value: function addFloat(value) {
                var indirect = arguments.length > 1 && arguments[1] !== void 0 ? arguments[1] : false, deduplicate = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : false;
                this.integrityCheckOnValueAddition();
                if (!indirect) {
                    this.stack.push(this.floatStackValue(value));
                    return;
                }
                if (deduplicate && Object.prototype.hasOwnProperty.call(this.indirectFloatLookup, value)) {
                    this.stack.push(this.indirectFloatLookup[value]);
                    return;
                }
                var stackValue = this.floatStackValue(value);
                var byteWidth = this.align(stackValue.width);
                var newOffset = this.computeOffset(byteWidth);
                var valueOffset = this.offset;
                stackValue.writeToBuffer(byteWidth);
                var stackOffset = this.offsetStackValue(valueOffset, ValueType.INDIRECT_FLOAT, stackValue.width);
                this.stack.push(stackOffset);
                this.offset = newOffset;
                if (deduplicate) {
                    this.indirectFloatLookup[value] = stackOffset;
                }
            }
        }
    ]);
    return Builder;
}();
