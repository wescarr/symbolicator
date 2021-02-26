use bytes::Buf;
use serde::de::DeserializeOwned;

use crate::utils::http::BadRequest;

const MAX_JSON_SIZE: usize = 1_000_000;

/*
TODO: Streaming implementation. Doesn't give much due to impl Buf copy

type BS = Box<dyn Stream<Item = Result<Bytes, warp::Error>> + Unpin>;
type MS = MultipartStream<BS, warp::Error>;

fn create_multipart<S, B>(mime: Mime, body: S) -> MS
where
    S: Stream<Item = Result<B, warp::Error>> + Unpin + 'static,
    B: Buf,
{
    let boundary = mime.get_param("boundary").map(|v| v.to_string()).unwrap();

    MultipartStream::new(
        boundary,
        Box::new(body.map_ok(|mut buf| buf.copy_to_bytes(buf.remaining()))),
    )
}

fn multipart() -> impl Filter<Extract = (MS,), Error = Rejection> + Clone {
    warp::any()
        .and(warp::header::<Mime>("content-type"))
        .and(warp::body::stream())
        .map(create_multipart)
}

async fn read_multipart_file2(
    field: MultipartField<BS, warp::Error>,
) -> Result<Vec<u8>, warp::Error> {
    let mut body = Vec::with_capacity(512);
    while let Some(bytes) = field.try_next().await? {
        body.extend_from_slice(&bytes);
    }
    Ok(body)
}
*/

pub async fn read_multipart_part(mut part: warp::multipart::Part) -> Result<Vec<u8>, BadRequest> {
    let mut vec = Vec::new();
    while let Some(result) = part.data().await {
        let mut buf = result?;
        while buf.has_remaining() {
            let chunk = buf.chunk();
            vec.extend_from_slice(chunk);
            let len = chunk.len();
            buf.advance(len);
        }
    }
    Ok(vec)
}

pub async fn read_multipart_json<T>(part: warp::multipart::Part) -> Result<T, BadRequest>
where
    T: DeserializeOwned,
{
    // TODO: MAX_JSON_SIZE
    let data = read_multipart_part(part).await?;
    Ok(serde_json::from_slice(&data)?)
}
