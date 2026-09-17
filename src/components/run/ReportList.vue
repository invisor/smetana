<script setup>
/* The Reports tab's page: every run report the project has, with the two
   controls a long list needs — how many rows a page holds, and which end of
   the list is read first — and the pagination under it. `reportsPage.js`
   carries the whole of the rule; this component only draws it.

   `page` is local and deliberately not a prop: nobody outside this component
   has any use for which page somebody is looking at, and it resets to 1
   whenever `rows`, `perPage` or `order` changes — a smaller page size, a
   flipped order or a freshly reread list all invalidate whatever page number
   was on screen, and holding it would show "page 4" over three pages of
   rows. */
import { computed, ref, watch } from 'vue'
import Select from '../core/Select.vue'
import IconButton from '../core/IconButton.vue'
import EmptyState from '../core/EmptyState.vue'
import Skeleton from '../core/Skeleton.vue'
import ReportRow from './ReportRow.vue'
import { COLUMNS, ORDER_CHOICES, PAGE_SIZE_CHOICES, pageOf } from './reportsPage.js'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  perPage: { type: Number, required: true },
  order: { type: String, required: true }
})

const emit = defineEmits(['update:perPage', 'update:order', 'open'])

/* A fixed width for the two `Select`s, so the row above the list does not
   reflow by a few pixels as somebody flips between "20 per page" and "100 per
   page". A geometry constant rather than a token, the same shape
   `sessionMenu.js`'s `SESSION_MENU_W` and `taskMenu.js`'s `MENU_W` already
   are: this is a control's own width, not a value the space scale or a
   density switch has any opinion about — `Select`'s *height* already scales
   through `--control-h-sm`, and its width is a separate question this
   design system has no token for. */
const SELECT_W = 160

const page = ref(1)

watch(
  () => [props.rows, props.perPage, props.order],
  () => {
    page.value = 1
  }
)

const paged = computed(() =>
  pageOf(props.rows, { order: props.order, perPage: props.perPage, page: page.value })
)

const prevDisabled = computed(() => paged.value.page <= 1)
const nextDisabled = computed(() => paged.value.page >= paged.value.pages)

function prevPage() {
  page.value = Math.max(1, paged.value.page - 1)
}
function nextPage() {
  page.value = Math.min(paged.value.pages, paged.value.page + 1)
}

/* The empty state is drawn only once loading has actually settled — the same
   rule `sessions.js`'s own tab follows, and for the same reason: a sentence
   claiming the disk holds nothing must not appear before anybody has looked
   at it. */
const showEmpty = computed(() => !props.loading && props.rows.length === 0)
const showSkeleton = computed(() => props.loading && props.rows.length === 0)
const showList = computed(() => props.rows.length > 0)

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  flex: 1,
  minWidth: 0,
  minHeight: 0
}
const controlsStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  padding: 'var(--panel-pad)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const selectWidthStyle = { width: `${SELECT_W}px` }
const bodyStyle = {
  flex: 1,
  minHeight: 0,
  overflow: 'auto'
}
const bodyPadStyle = { padding: 'var(--panel-pad)' }
const headerStyle = {
  display: 'grid',
  gridTemplateColumns: COLUMNS,
  gap: 'var(--space-5)',
  padding: '0 var(--space-4) var(--space-2)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const headerRightStyle = { textAlign: 'right' }
const footerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  padding: 'var(--panel-pad)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-secondary)'
}
const totalStyle = { marginLeft: 'auto', color: 'var(--text-muted)' }
</script>

<template>
  <div :style="rootStyle">
    <div :style="controlsStyle">
      <Select
        size="sm"
        :style="selectWidthStyle"
        :model-value="perPage"
        :options="PAGE_SIZE_CHOICES"
        @update:model-value="emit('update:perPage', Number($event))"
      />
      <Select
        size="sm"
        :style="selectWidthStyle"
        :model-value="order"
        :options="ORDER_CHOICES"
        @update:model-value="emit('update:order', $event)"
      />
    </div>
    <div :style="bodyStyle">
      <div v-if="showSkeleton" :style="bodyPadStyle">
        <Skeleton :lines="6" :height="16" />
      </div>
      <EmptyState
        v-else-if="showEmpty"
        icon="scroll-text"
        title="No run reports yet"
        description="A run writes one when it ends."
      />
      <template v-else-if="showList">
        <div :style="headerStyle">
          <span>When</span>
          <span>Report</span>
          <span>Scope</span>
          <span :style="headerRightStyle">Closed</span>
          <span :style="headerRightStyle">Parked</span>
          <span :style="headerRightStyle">Batches</span>
          <span :style="headerRightStyle">Total</span>
        </div>
        <ReportRow
          v-for="row in paged.rows"
          :key="row.path"
          :row="row"
          @open="emit('open', $event)"
        />
      </template>
    </div>
    <div v-if="showList" :style="footerStyle">
      <IconButton
        icon="chevron-left"
        label="Previous page"
        size="sm"
        :disabled="prevDisabled"
        @click="prevPage"
      />
      <span>Page {{ paged.page }} of {{ paged.pages }}</span>
      <IconButton
        icon="chevron-right"
        label="Next page"
        size="sm"
        :disabled="nextDisabled"
        @click="nextPage"
      />
      <span :style="totalStyle">{{ paged.total }} reports</span>
    </div>
  </div>
</template>
