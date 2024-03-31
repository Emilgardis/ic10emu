

function docReady(fn) {
  // see if DOM is already available
  if (document.readyState === "complete" || document.readyState === "interactive") {
    setTimeout(fn, 1);
  } else {
    document.addEventListener("DOMContentLoaded", fn);
  }
}

// probably not needed, fetch() exists now
function makeRequest (opts) {
  const req = new Request()
  return new Promise(function (resolve, reject) {
    var xhr = new XMLHttpRequest();
    xhr.open(opts.method, opts.url);
    xhr.onload = function () {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(xhr.response);
      } else {
        reject({
          status: xhr.status,
          statusText: xhr.statusText
        });
      }
    };
    xhr.onerror = function () {
      reject({
        status: xhr.status,
        statusText: xhr.statusText
      });
    };
    if (opts.headers) {
      Object.keys(opts.headers).forEach(function (key) {
        xhr.setRequestHeader(key, opts.headers[key]);
      });
    }
    var params = opts.params;
    if (params && typeof params === 'object') {
      params = Object.keys(params).map(function (key) {
        return encodeURIComponent(key) + '=' + encodeURIComponent(params[key]);
      }).join('&');
    }
    xhr.send(params);
  });
}

async function saveFile(content) {
  const blob = new Blob([content], { type: "text/plain" });
  if (typeof window.showSaveFilePicker !== "undefined") {
    console.log("Saving via FileSystem API")
    const options = {
      types: [
        {
          suggestedName: "code.ic10",
          description: 'Text Files',
          accept: {
            'text/plain': ['.txt', '.ic10'],
          },
        },
      ],
    };
    const saveHandle = await window.showSaveFilePicker(options);
    const ws = await saveHandle.createWritable();
    await ws.write(blob);
    await ws.close();
  } else {
    console.log("saving file via hidden link event");
    var a = document.createElement('a');
    a.download = "code.ic10";
    a.href = window.URL.createObjectURL(blob);
    a.click();
  }
}

async function openFile(editor) {
  if (typeof window.showOpenFilePicker !== "undefined") {
    console.log("opening file via FileSystem Api");
    const [fileHandle] = await window.showOpenFilePicker();
    const file = await fileHandle.getFile();
    const contents = await file.text();
    const session = editor.getSession();
    session.setValue(contents);
  } else {
    console.log("opening file via hidden input event");
    let input = document.createElement('input');
    input.type = 'file';
    input.accept = ".txt,.ic10,.mips,text/*";
    input.onchange = _ => {
      const files = Array.from(input.files);
      console.log(files);
      const file = files[0];
      var reader = new FileReader();
      reader.onload = (e) => {
        const contents = e.target.result;
        const session = editor.getSession();
        // session.id = file.name;
        session.setValue(contents);
      };
      reader.readAsText(file);
    };
    input.click();
  }
}
export { docReady, makeRequest, saveFile, openFile }