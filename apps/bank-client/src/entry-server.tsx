// @refresh reload
import { createHandler, StartServer } from "@solidjs/start/server";

export default createHandler(() => (
  <StartServer
    document={({ assets, children, scripts }) => (
      <html lang="en">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <link rel="icon" href="/favicon.ico" />
          {assets}
        </head>
        <body>
          <script>{`(function(){try{var t=localStorage.getItem('w3b-theme');var r=t==='light'||(t==='system'||!t)&&window.matchMedia('(prefers-color-scheme:light)').matches;if(r)document.documentElement.classList.add('light')}catch(e){}})()`}</script>
          <div id="app">{children}</div>
          {scripts}
        </body>
      </html>
    )}
  />
));
