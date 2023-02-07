import urlJoin from "url-join";
import { decode } from "@msgpack/msgpack";
import { retryFetch } from "./fetch";

const CDN_ROOT: string = process.env.CDN_ROOT || "http://localhost:8080";

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function encodeBase64(payload) {
  return Buffer.from(payload).toString("base64");
}

export type V2Module = Record<string, Buffer>;

export async function fetchV2Module(
  name: string,
  version: string
): Promise<V2Module> {
  const specifier = `${name}@${version}`;
  const encoded_specifier = encodeBase64(specifier);
  const result = await retryFetch(
    urlJoin(CDN_ROOT, `/v2/mod/${encoded_specifier}`),
    { maxRetries: 5 }
  );
  // @ts-ignore
  const blob = await result.buffer();
  return decode(blob) as V2Module;
}

export type V2Deps = Record<string, string>;

export async function fetchV2Deps(
  deps: Array<{name: string, range: string}>
): Promise<V2Deps> {
  const specifier = deps.map(v => `${v.name}@${v.range}`).join(';');
  const encoded_specifier = encodeBase64(specifier);
  const url = urlJoin(CDN_ROOT, `/v2/deps/${encoded_specifier}`);
  const result = await retryFetch(
    url,
    { maxRetries: 5 }
  );
  // @ts-ignore
  const blob = await result.buffer();
  return decode(blob) as V2Deps;
}
