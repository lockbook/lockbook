package app.lockbook.screen

import android.animation.ValueAnimator
import android.content.Context
import android.os.Build
import android.provider.Settings
import android.view.ContextThemeWrapper
import androidx.dynamicanimation.animation.DynamicAnimation
import androidx.dynamicanimation.animation.SpringAnimation
import androidx.dynamicanimation.animation.SpringForce
import androidx.recyclerview.widget.RecyclerView
import androidx.recyclerview.widget.SimpleItemAnimator
import app.lockbook.R
import com.google.android.material.motion.MotionUtils
import com.google.android.material.R as MaterialR

/** Expressive spatial springs reposition rows; non-overshooting effects springs fade them. */
internal class RecentsItemAnimator(
    context: Context,
) : SimpleItemAnimator() {
    private enum class Kind { Add, Remove, Move }

    private class Transition(
        val kind: Kind,
    ) {
        val springs = mutableListOf<SpringAnimation>()
    }

    private val motionContext = ContextThemeWrapper(context, R.style.RecentsMotion)
    private val spatial =
        MotionUtils.resolveThemeSpringForce(
            motionContext,
            MaterialR.attr.motionSpringDefaultSpatial,
            MaterialR.style.Motion_Material3_Spring_Expressive_Default_Spatial,
        )
    private val effects =
        MotionUtils.resolveThemeSpringForce(
            motionContext,
            MaterialR.attr.motionSpringFastEffects,
            MaterialR.style.Motion_Material3_Spring_Expressive_Fast_Effects,
        )
    private val pending = linkedMapOf<RecyclerView.ViewHolder, Transition>()
    private val running = mutableMapOf<RecyclerView.ViewHolder, Transition>()

    init {
        // Selection and section-shape rebinds should update in place, without crossfading rows.
        supportsChangeAnimations = false
    }

    override fun animateAdd(holder: RecyclerView.ViewHolder): Boolean {
        val interruptedFade = (pending[holder] ?: running[holder])?.kind in setOf(Kind.Add, Kind.Remove)
        val alpha = if (interruptedFade) holder.itemView.alpha else 0f
        endAnimation(holder)
        holder.itemView.alpha = alpha
        pending[holder] = Transition(Kind.Add)
        return true
    }

    override fun animateRemove(holder: RecyclerView.ViewHolder): Boolean {
        val alpha = holder.itemView.alpha
        endAnimation(holder)
        holder.itemView.alpha = alpha
        pending[holder] = Transition(Kind.Remove)
        return true
    }

    override fun animateMove(
        holder: RecyclerView.ViewHolder,
        fromX: Int,
        fromY: Int,
        toX: Int,
        toY: Int,
    ): Boolean {
        // Preserve the visual position if another filter interrupts an in-flight spring.
        val x = fromX + holder.itemView.translationX - toX
        val y = fromY + holder.itemView.translationY - toY
        endAnimation(holder)
        if (x == 0f && y == 0f) {
            dispatchMoveFinished(holder)
            return false
        }
        holder.itemView.translationX = x
        holder.itemView.translationY = y
        pending[holder] = Transition(Kind.Move)
        return true
    }

    override fun animateChange(
        oldHolder: RecyclerView.ViewHolder,
        newHolder: RecyclerView.ViewHolder?,
        fromX: Int,
        fromY: Int,
        toX: Int,
        toY: Int,
    ): Boolean {
        if (oldHolder === newHolder) return animateMove(oldHolder, fromX, fromY, toX, toY)
        endAnimation(oldHolder)
        dispatchChangeFinished(oldHolder, true)
        newHolder?.let {
            endAnimation(it)
            dispatchChangeFinished(it, false)
        }
        return false
    }

    override fun runPendingAnimations() {
        val batch = pending.toMap()
        pending.clear()
        running.putAll(batch)
        // Start exits, entrances and gap closing together, with no stagger or blocking delays.
        batch.forEach { (holder, transition) ->
            when (transition.kind) {
                Kind.Add -> dispatchAddStarting(holder)
                Kind.Remove -> dispatchRemoveStarting(holder)
                Kind.Move -> dispatchMoveStarting(holder)
            }
            if (!animationsEnabled()) {
                finish(holder, transition)
                return@forEach
            }
            when (transition.kind) {
                Kind.Add -> {
                    transition.springs.add(spring(holder, DynamicAnimation.ALPHA, 1f, effects))
                }

                Kind.Remove -> {
                    transition.springs.add(spring(holder, DynamicAnimation.ALPHA, 0f, effects))
                }

                Kind.Move -> {
                    if (holder.itemView.translationX !=
                        0f
                    ) {
                        transition.springs.add(spring(holder, DynamicAnimation.TRANSLATION_X, 0f, spatial))
                    }
                    if (holder.itemView.translationY !=
                        0f
                    ) {
                        transition.springs.add(spring(holder, DynamicAnimation.TRANSLATION_Y, 0f, spatial))
                    }
                }
            }
            transition.springs.forEach { animation ->
                animation.addEndListener { _, _, _, _ ->
                    if (running[holder] === transition && transition.springs.none { it.isRunning }) {
                        finish(holder, transition)
                    }
                }
                animation.start()
            }
        }
    }

    private fun animationsEnabled(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ValueAnimator.areAnimatorsEnabled()
        } else {
            Settings.Global.getFloat(motionContext.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) != 0f
        }

    private fun spring(
        holder: RecyclerView.ViewHolder,
        property: DynamicAnimation.ViewProperty,
        target: Float,
        spec: SpringForce,
    ): SpringAnimation =
        SpringAnimation(holder.itemView, property, target).apply {
            spring = SpringForce(target).setDampingRatio(spec.dampingRatio).setStiffness(spec.stiffness)
        }

    private fun finish(
        holder: RecyclerView.ViewHolder,
        transition: Transition,
    ) {
        running.remove(holder)
        holder.itemView.apply {
            alpha = 1f
            translationX = 0f
            translationY = 0f
        }
        when (transition.kind) {
            Kind.Add -> dispatchAddFinished(holder)
            Kind.Remove -> dispatchRemoveFinished(holder)
            Kind.Move -> dispatchMoveFinished(holder)
        }
        if (!isRunning) dispatchAnimationsFinished()
    }

    override fun endAnimation(item: RecyclerView.ViewHolder) {
        val transition = pending.remove(item) ?: running.remove(item) ?: return
        // Remove ownership before cancelling: spring end callbacks must not finish this twice.
        transition.springs.forEach { it.cancel() }
        finish(item, transition)
    }

    override fun endAnimations() {
        (pending.keys + running.keys).toList().forEach(::endAnimation)
    }

    override fun isRunning(): Boolean = pending.isNotEmpty() || running.isNotEmpty()
}
