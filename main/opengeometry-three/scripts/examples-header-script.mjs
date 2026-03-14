const MIXPANEL_MARKER = "<!-- Mixpanel -->";
const MIXPANEL_TOKEN = "46cc87ad3cea4d093ec0bc61b0b774d9";
const MIXPANEL_HOST = "https://api-eu.mixpanel.com";

function normalizePagePath(pagePath) {
  if (!pagePath || pagePath === "/") {
    return "/index.html";
  }

  return pagePath.endsWith(".html") ? pagePath : `${pagePath}.html`;
}

function createMixpanelSnippet(pagePath) {
  return `<!-- Mixpanel -->
<script type="text/javascript">
  (function(e,c){if(!c.__SV){var l,h;window.mixpanel=c;c._i=[];c.init=function(q,r,f){function t(d,a){var g=a.split(".");2==g.length&&(d=d[g[0]],a=g[1]);d[a]=function(){d.push([a].concat(Array.prototype.slice.call(arguments,0)))}}var b=c;"undefined"!==typeof f?b=c[f]=[]:f="mixpanel";b.people=b.people||[];b.toString=function(d){var a="mixpanel";"mixpanel"!==f&&(a+="."+f);d||(a+=" (stub)");return a};b.people.toString=function(){return b.toString(1)+".people (stub)"};l="disable time_event track track_pageview track_links track_forms track_with_groups add_group set_group remove_group register register_once alias unregister identify name_tag set_config reset opt_in_tracking opt_out_tracking has_opted_in_tracking has_opted_out_tracking clear_opt_in_out_tracking start_batch_senders start_session_recording stop_session_recording people.set people.set_once people.unset people.increment people.append people.union people.track_charge people.clear_charges people.delete_user people.remove".split(" ");
  for(h=0;h<l.length;h++)t(b,l[h]);var n="set set_once union unset remove delete".split(" ");b.get_group=function(){function d(p){a[p]=function(){b.push([g,[p].concat(Array.prototype.slice.call(arguments,0))])}}for(var a={},g=["get_group"].concat(Array.prototype.slice.call(arguments,0)),m=0;m<n.length;m++)d(n[m]);return a};c._i.push([q,r,f])};c.__SV=1.2;var k=e.createElement("script");k.type="text/javascript";k.async=!0;k.src="undefined"!==typeof MIXPANEL_CUSTOM_LIB_URL?MIXPANEL_CUSTOM_LIB_URL:"file:"===
  e.location.protocol&&"//cdn.mxpnl.com/libs/mixpanel-2-latest.min.js".match(/^\\/\\//)?"https://cdn.mxpnl.com/libs/mixpanel-2-latest.min.js":"//cdn.mxpnl.com/libs/mixpanel-2-latest.min.js";e=e.getElementsByTagName("script")[0];e.parentNode.insertBefore(k,e)}})(document,window.mixpanel||[]);

  mixpanel.init('${MIXPANEL_TOKEN}', {
    autocapture: true,
    track_pageview: true,
    record_sessions_percent: 100,
    api_host: '${MIXPANEL_HOST}',
  });

  (function () {
    var pagePath = ${JSON.stringify(pagePath)};
    var pageName = pagePath.split("/").pop().replace(/\\.html$/, "") || "index";
    var pageSection = pagePath.split("/").length > 2 ? pagePath.split("/")[1] : "catalog";
    var distinctId = null;
    var pageProperties = {
      example_name: pageName,
      example_path: pagePath,
      example_section: pageSection,
      page_title: document.title,
    };

    if (typeof mixpanel !== "undefined" && typeof mixpanel.get_distinct_id === "function") {
      distinctId = mixpanel.get_distinct_id();
    }

    if (distinctId && typeof mixpanel !== "undefined" && typeof mixpanel.identify === "function") {
      mixpanel.identify(distinctId);
    }

    if (typeof mixpanel !== "undefined" && typeof mixpanel.register === "function") {
      mixpanel.register(pageProperties);
    }

    if (
      distinctId
      && typeof mixpanel !== "undefined"
      && typeof mixpanel.people === "object"
      && typeof mixpanel.people.set_once === "function"
    ) {
      mixpanel.people.set_once({
        first_example_name: pageName,
        first_example_path: pagePath,
        first_example_section: pageSection,
        first_seen_page_title: document.title,
      });
    }

    if (typeof mixpanel !== "undefined" && typeof mixpanel.track === "function") {
      mixpanel.track("Example Loaded", pageProperties);
    }

    function normalizeLabel(value) {
      if (!value) {
        return null;
      }

      var normalized = value.replace(/\\s+/g, " ").trim();
      if (!normalized) {
        return null;
      }

      return normalized.length > 80 ? normalized.slice(0, 80) + "..." : normalized;
    }

    document.addEventListener("click", function (event) {
      if (typeof mixpanel === "undefined" || typeof mixpanel.track !== "function") {
        return;
      }

      var target = event.target instanceof Element ? event.target.closest("button, a") : null;
      if (!target) {
        return;
      }

      mixpanel.track("Example Button Clicked", {
        example_name: pageName,
        example_path: pagePath,
        example_section: pageSection,
        button_label: normalizeLabel(target.textContent),
        button_id: target.id || null,
        button_href: target.tagName === "A" ? target.getAttribute("href") || null : null,
        button_role: target.getAttribute("role") || null,
      });
    }, true);
  })();
</script>`;
}

export function createExamplesHeaderPlugin() {
  return {
    name: "opengeometry-examples-mixpanel",
    apply: "build",
    transformIndexHtml(html, context) {
      if (html.includes("mixpanel.init(")) {
        return html;
      }

      const pagePath = normalizePagePath(context?.path);
      const snippet = createMixpanelSnippet(pagePath);

      if (html.includes(MIXPANEL_MARKER)) {
        return html.replace(MIXPANEL_MARKER, snippet);
      }

      if (!html.includes("</head>")) {
        return html;
      }

      return html.replace("</head>", `${snippet}\n  </head>`);
    },
  };
}

export const createExamplesMixpanelPlugin = createExamplesHeaderPlugin;
