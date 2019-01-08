/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::fmt;
use std::time::Duration;

pub use reqwest::{get, header, Error, IntoUrl, Method, Request, Response, StatusCode, UrlError};
use http::{HttpTryFrom};

use reqwest::{Body, Client};
use serde::{Serialize};

#[derive(Clone, Debug)]
pub struct HttpClient {
    inner: reqwest::Client,
}

impl HttpClient {
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        // // let req = url
        // //     .into_url()
        // //     .map(move |url| Request::new(method, url));
        RequestBuilder { inner: self.inner.request(method, url) }

        // let foo: i32 = self.inner.request(method, url);
    }

    pub fn fetch(&self, request: ::reqwest::Request) -> ::reqwest::Result<Response> {
        self.inner.execute(request)
    }
}

/// A builder to construct the properties of a `Request`.
#[derive(Debug)]
pub struct HttpClientBuilder {
    inner: reqwest::ClientBuilder,
}

// pub fn builder() -> RequestBuilder {
impl HttpClientBuilder {
    pub fn new() -> HttpClientBuilder {
        HttpClientBuilder { inner: reqwest::ClientBuilder::new() }
    }

    pub fn build(self) -> ::reqwest::Result<HttpClient> {
        self.inner.build().map(|inner| HttpClient { inner })
    }

    /// Set a timeout for connect, read and write operations of a `Client`.
    ///
    /// Default is 30 seconds.
    ///
    /// Pass `None` to disable timeout.
    pub fn timeout<T>(mut self, timeout: T) -> HttpClientBuilder
    where T: Into<Option<Duration>>,
    {
        self.inner = self.inner.timeout(timeout);
        self
    }
}

/// A builder to construct the properties of a `Request`.
#[derive(Debug)]
pub struct RequestBuilder {
    inner: reqwest::RequestBuilder,
}

// pub fn builder() -> RequestBuilder {
impl RequestBuilder {
    pub fn post<U: IntoUrl>(url: U) -> RequestBuilder {
        // XXX
        let client = Client::new();
        RequestBuilder { inner: client.post(url) }
    }

    pub fn get<U: IntoUrl>(url: U) -> RequestBuilder {
        // XXX
        let client = Client::new();
        RequestBuilder { inner: client.get(url) }
    }

 //    pub(crate) fn new(request: ::reqwest::Result<Request>) -> RequestBuilder {
 //        // XXX
 //        let client = Client::new();
        

 //        RequestBuilder {
 //            inner: client.

 // reqwest::RequestBuilder {
 //                client,
 //                request,
 //            }
 //        }
 //    }

    fn with_inner<F>(mut self, func: F) -> RequestBuilder
    where
        F: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        self.inner = func(self.inner);
        self
    }

    /// Add a `Header` to this Request.
    ///
    /// ```rust
    /// use reqwest::header::USER_AGENT;
    ///
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let client = reqwest::Client::new();
    /// let res = client.get("https://www.rust-lang.org")
    ///     .header(USER_AGENT, "foo")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn header<K, V>(self, key: K, value: V) -> RequestBuilder
    where
        reqwest::header::HeaderName: HttpTryFrom<K>,
        reqwest::header::HeaderValue: HttpTryFrom<V>,
    {
        self.with_inner(|inner| inner.header(key, value))
    }

    /// Add a set of Headers to the existing ones on this Request.
    ///
    /// The headers will be merged in to any already set.
    ///
    /// ```rust
    /// use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, CONTENT_TYPE};
    /// # use std::fs;
    ///
    /// fn construct_headers() -> HeaderMap {
    ///     let mut headers = HeaderMap::new();
    ///     headers.insert(USER_AGENT, HeaderValue::from_static("reqwest"));
    ///     headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
    ///     headers
    /// }
    ///
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let file = fs::File::open("much_beauty.png")?;
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .headers(construct_headers())
    ///     .body(file)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn headers(self, headers: ::reqwest::header::HeaderMap) -> RequestBuilder {
        self.with_inner(|inner| inner.headers(headers))
    }

    // /// Set a header with a type implementing hyper v0.11's `Header` trait.
    // ///
    // /// This method is provided to ease migration, and requires the `hyper-011`
    // /// Cargo feature enabled on `reqwest`.
    // #[cfg(feature = "hyper-011")]
    // pub fn header_011<H>(self, header: H) -> RequestBuilder
    // where
    //     H: ::hyper_011::reqwest::header::header,
    // {
    //     let mut headers = ::hyper_011::reqwest::headers::new();
    //     headers.set(header);
    //     let map = ::reqwest::header::headerMap::from(headers);
    //     self.headers(map)
    // }

    // /// Set multiple headers using hyper v0.11's `Headers` map.
    // ///
    // /// This method is provided to ease migration, and requires the `hyper-011`
    // /// Cargo feature enabled on `reqwest`.
    // #[cfg(feature = "hyper-011")]
    // pub fn headers_011(self, headers: ::hyper_011::reqwest::headers) -> RequestBuilder {
    //     let map = ::reqwest::header::headerMap::from(headers);
    //     self.headers(map)
    // }

    /// Enable HTTP basic authentication.
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let client = reqwest::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .basic_auth("admin", Some("good password"))
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> RequestBuilder
    where
        U: fmt::Display,
        P: fmt::Display,
    {
        self.with_inner(|inner| inner.basic_auth(username, password))
    }

    /// Enable HTTP bearer authentication.
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let client = reqwest::Client::new();
    /// let resp = client.delete("http://httpbin.org/delete")
    ///     .bearer_auth("token")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn bearer_auth<T>(self, token: T) -> RequestBuilder
    where
        T: fmt::Display,
    {
        self.with_inner(|inner| inner.bearer_auth(token))
    }

    /// Set the request body.
    ///
    /// # Examples
    ///
    /// Using a string:
    ///
    /// ```rust
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body("from a &str!")
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Using a `File`:
    ///
    /// ```rust
    /// # use std::fs;
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// let file = fs::File::open("from_a_file.txt")?;
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body(file)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Using arbitrary bytes:
    ///
    /// ```rust
    /// # use std::fs;
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// // from bytes!
    /// let bytes: Vec<u8> = vec![1, 10, 100];
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org/post")
    ///     .body(bytes)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn body<T: Into<Body>>(self, body: T) -> RequestBuilder {
        self.with_inner(|inner| inner.body(body))
    }

    /// Modify the query string of the URL.
    ///
    /// Modifies the URL of this request, adding the parameters provided.
    /// This method appends and does not overwrite. This means that it can
    /// be called multiple times and that existing query parameters are not
    /// overwritten if the same key is used. The key will simply show up
    /// twice in the query string.
    /// Calling `.query(&[("foo", "a"), ("foo", "b")])` gives `"foo=a&foo=b"`.
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let client = reqwest::Client::new();
    /// let res = client.get("http://httpbin.org")
    ///     .query(&[("lang", "rust")])
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Note
    /// This method does not support serializing a single key-value
    /// pair. Instead of using `.query(("key", "val"))`, use a sequence, such
    /// as `.query(&[("key", "val")])`. It's also possible to serialize structs
    /// and maps into a key-value pair.
    ///
    /// # Errors
    /// This method will fail if the object you provide cannot be serialized
    /// into a query string.
    pub fn query<T: Serialize + ?Sized>(self, query: &T) -> RequestBuilder {
        self.with_inner(|inner| inner.query(query))
    }

    /// Send a form body.
    ///
    /// Sets the body to the url encoded serialization of the passed value,
    /// and also sets the `Content-Type: application/x-www-form-urlencoded`
    /// header.
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// # use std::collections::HashMap;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let mut params = HashMap::new();
    /// params.insert("lang", "rust");
    ///
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .form(&params)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// This method fails if the passed value cannot be serialized into
    /// url encoded format
    pub fn form<T: Serialize + ?Sized>(self, form: &T) -> RequestBuilder {
        self.with_inner(|inner| inner.form(form))
    }

    /// Send a JSON body.
    ///
    /// Sets the body to the JSON serialization of the passed value, and
    /// also sets the `Content-Type: application/json` header.
    ///
    /// ```rust
    /// # use reqwest::Error;
    /// # use std::collections::HashMap;
    /// #
    /// # fn run() -> Result<(), Error> {
    /// let mut map = HashMap::new();
    /// map.insert("lang", "rust");
    ///
    /// let client = reqwest::Client::new();
    /// let res = client.post("http://httpbin.org")
    ///     .json(&map)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Serialization can fail if `T`'s implementation of `Serialize` decides to
    /// fail, or if `T` contains a map with non-string keys.
    pub fn json<T: Serialize + ?Sized>(self, json: &T) -> RequestBuilder {
        self.with_inner(|inner| inner.json(json))
    }

    /// Sends a multipart/form-data body.
    ///
    /// ```
    /// # use reqwest::Error;
    ///
    /// # fn run() -> Result<(), Box<std::error::Error>> {
    /// let client = reqwest::Client::new();
    /// let form = reqwest::multipart::Form::new()
    ///     .text("key3", "value3")
    ///     .file("file", "/path/to/field")?;
    ///
    /// let response = client.post("your url")
    ///     .multipart(form)
    ///     .send()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// See [`multipart`](multipart/) for more examples.
    pub fn multipart(self, multipart: ::reqwest::multipart::Form) -> RequestBuilder {
        self.with_inner(|inner| inner.multipart(multipart))
    }

    /// Build a `Request`, which can be inspected, modified and executed with
    /// `Client::execute()`.
    pub fn build(self) -> ::reqwest::Result<Request> {
        self.inner.build()
    }

    // /// Constructs the Request and sends it the target URL, returning a Response.
    // ///
    // /// # Errors
    // ///
    // /// This method fails if there was an error while sending request,
    // /// redirect loop was detected or redirect limit was exhausted.
    // pub fn send(self) -> ::reqwest::Result<Response> {
    //     self.inner.send()
    // }

    // /// Attempts to clone the `RequestBuilder`.
    // ///
    // /// None is returned if a body is which can not be cloned. This can be because the body is a
    // /// stream.
    // ///
    // /// # Examples
    // ///
    // /// With a static body
    // ///
    // /// ```rust
    // /// # fn run() -> Result<(), Box<::std::error::Error>> {
    // /// let client = reqwest::Client::new();
    // /// let builder = client.post("http://httpbin.org/post")
    // ///     .body("from a &str!");
    // /// let clone = builder.try_clone();
    // /// assert!(clone.is_some());
    // /// # Ok(())
    // /// # }
    // /// ```
    // ///
    // /// Without a body
    // ///
    // /// ```rust
    // /// # fn run() -> Result<(), Box<::std::error::Error>> {
    // /// let client = reqwest::Client::new();
    // /// let builder = client.get("http://httpbin.org/get");
    // /// let clone = builder.try_clone();
    // /// assert!(clone.is_some());
    // /// # Ok(())
    // /// # }
    // /// ```
    // ///
    // /// With a non-clonable body
    // ///
    // /// ```rust
    // /// # fn run() -> Result<(), Box<::std::error::Error>> {
    // /// let client = reqwest::Client::new();
    // /// let builder = client.get("http://httpbin.org/get")
    // ///     .body(reqwest::Body::new(std::io::empty()));
    // /// let clone = builder.try_clone();
    // /// assert!(clone.is_none());
    // /// # Ok(())
    // /// # }
    // /// ```
    // pub fn try_clone(&self) -> Option<RequestBuilder> {
    //     self.inner.try_clone().map(|inner| RequestBuilder { inner })
    //     // self.request.as_ref()
    //     //     .ok()
    //     //     .and_then(|req| req.try_clone())
    //     //     .map(|req| {
    //     //         RequestBuilder{
    //     //             client: self.client.clone(),
    //     //             request: Ok(req),
    //     //         }
    //     //     })
    // }
}
