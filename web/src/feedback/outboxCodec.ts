/** Request bodies are kept as JSON text, gzip-compressed (base64) when large and the browser can. */
export interface EncodedBody {
  enc: "json" | "gzip";
  data: string;
}

export const COMPRESS_OVER_BYTES = 32 * 1024;

const toBase64 = (bytes: Uint8Array): string => {
  let out = "";
  for (let i = 0; i < bytes.length; i += 0x8000) out += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(out);
};
const fromBase64 = (text: string): Uint8Array => Uint8Array.from(atob(text), (c) => c.charCodeAt(0));

async function pipe(bytes: Uint8Array, stream: { readable: ReadableStream; writable: WritableStream }): Promise<Uint8Array> {
  const writer = stream.writable.getWriter();
  void writer.write(bytes as unknown as BufferSource).then(() => writer.close());
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}

export async function encodeBody(json: string): Promise<EncodedBody> {
  if (json.length > COMPRESS_OVER_BYTES && typeof CompressionStream !== "undefined") {
    try {
      const packed = await pipe(new TextEncoder().encode(json), new CompressionStream("gzip"));
      return { enc: "gzip", data: toBase64(packed) };
    } catch {
      // stored plain
    }
  }
  return { enc: "json", data: json };
}

export async function decodeBody(body: EncodedBody): Promise<string> {
  if (body.enc === "json") return body.data;
  const raw = await pipe(fromBase64(body.data), new DecompressionStream("gzip"));
  return new TextDecoder().decode(raw);
}
