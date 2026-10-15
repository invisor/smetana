/* The phone's `stores/windowDrops.js`: a subscription to nothing. A phone has no file to
   drag over a window. The shape is the desktop's - it hands back the function that
   unsubscribes - so a caller treats every environment the same way. */
export function watchWindowDrops() {
  return () => {}
}
