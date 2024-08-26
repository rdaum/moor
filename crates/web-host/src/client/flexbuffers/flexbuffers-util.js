export function fromUTF8Array(data) {
    var decoder = new TextDecoder();
    return decoder.decode(data);
}
export function toUTF8Array(str) {
    var encoder = new TextEncoder();
    return encoder.encode(str);
}
